use std::collections::VecDeque;

use alloy_primitives::{Address, U256};
use serde_json::{json, Value};

use crate::{
    config::HistorySourceConfig,
    error::{AppError, AppResult},
    history::{normalize::group_swaps_by_block, source::SwapInputSource},
    types::{BlockSwaps, SwapRequest},
};

#[derive(Debug, Default)]
pub struct ForkReplaySource {
    blocks: VecDeque<BlockSwaps>,
}

impl ForkReplaySource {
    pub fn from_config(config: &HistorySourceConfig) -> AppResult<Self> {
        match config {
            HistorySourceConfig::ForkReplay {
                rpc_url,
                start_block,
                end_block,
                router_address,
            } => Self::fetch(rpc_url, *start_block, *end_block, router_address),
            other => Err(AppError::validation(format!(
                "expected fork_replay history config, got `{}`",
                other.kind()
            ))),
        }
    }

    pub fn fetch(
        rpc_url: &str,
        start_block: u64,
        end_block: u64,
        router_address: &str,
    ) -> AppResult<Self> {
        let router_lc = router_address.to_ascii_lowercase();
        let mut swaps: Vec<SwapRequest> = Vec::new();

        for block_number in start_block..=end_block {
            let block = rpc_request(
                rpc_url,
                "eth_getBlockByNumber",
                json!([format!("0x{block_number:x}"), true]),
            )?;

            if block.is_null() {
                continue;
            }

            let txs = block
                .get("transactions")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    AppError::backend(format!(
                        "block {block_number} response is missing `transactions`"
                    ))
                })?;

            for tx in txs {
                let tx_hash = tx
                    .get("hash")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                // Filter: only transactions sent to the router
                let to = match tx.get("to").and_then(Value::as_str) {
                    Some(t) => t.to_ascii_lowercase(),
                    None => continue, // contract creation — skip
                };

                if to != router_lc {
                    continue;
                }

                // Decode calldata
                let input_hex = tx.get("input").and_then(Value::as_str).unwrap_or("0x");
                let calldata = decode_hex_bytes(input_hex).map_err(|e| {
                    AppError::validation(format!(
                        "failed to decode input for tx `{tx_hash}`: {e}"
                    ))
                })?;

                let decoded = match decode_v2_swap(&calldata)? {
                    Some(d) => d,
                    None => continue, // not a V2 swap selector — skip
                };

                // Parse tx fields
                let from = tx
                    .get("from")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                let tx_index = tx
                    .get("transactionIndex")
                    .and_then(Value::as_str)
                    .map(parse_hex_u64)
                    .transpose()
                    .map_err(|e| {
                        AppError::validation(format!(
                            "failed to parse transactionIndex for `{tx_hash}`: {e}"
                        ))
                    })?;

                // For ETH-in swaps the amount_in comes from msg.value
                let amount_in = if decoded.eth_in {
                    let value_hex = tx.get("value").and_then(Value::as_str).unwrap_or("0x0");
                    parse_hex_u256_str(value_hex).map_err(|e| {
                        AppError::validation(format!(
                            "failed to parse value for tx `{tx_hash}`: {e}"
                        ))
                    })?
                } else {
                    decoded.amount_in.to_string()
                };

                let swap = build_swap_request(
                    block_number,
                    &from,
                    &decoded,
                    amount_in,
                    tx_hash,
                    tx_index,
                );

                swaps.push(swap);
            }
        }

        let blocks = group_swaps_by_block(swaps);
        for block in &blocks {
            block.validate()?;
        }

        Ok(Self {
            blocks: VecDeque::from(blocks),
        })
    }

    pub fn remaining_blocks(&self) -> usize {
        self.blocks.len()
    }
}

impl SwapInputSource for ForkReplaySource {
    fn next_block(&mut self) -> AppResult<Option<BlockSwaps>> {
        Ok(self.blocks.pop_front())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_swap_request(
    block_number: u64,
    from: &str,
    decoded: &DecodedV2Swap,
    amount_in: String,
    tx_hash: String,
    tx_index: Option<u64>,
) -> SwapRequest {
    use std::collections::BTreeMap;

    let mut metadata = BTreeMap::new();
    metadata.insert("history_source".to_string(), "fork_replay".to_string());
    metadata.insert("decoded_method".to_string(), decoded.method.to_string());
    metadata.insert(
        "path".to_string(),
        decoded
            .path
            .iter()
            .map(|a| format!("{a:?}"))
            .collect::<Vec<_>>()
            .join("->"),
    );
    if decoded.eth_in {
        metadata.insert("eth_in".to_string(), "true".to_string());
    }
    if decoded.eth_out {
        metadata.insert("eth_out".to_string(), "true".to_string());
    }

    SwapRequest {
        block_number,
        sender: from.to_string(),
        token_in: format!("{:?}", decoded.token_in),
        token_out: format!("{:?}", decoded.token_out),
        amount_in,
        min_amount_out: decoded.amount_out_min.to_string(),
        tx_hash: Some(tx_hash),
        tx_index,
        metadata,
    }
}

fn rpc_request(rpc_url: &str, method: &str, params: Value) -> AppResult<Value> {
    let response = ureq::post(rpc_url)
        .send_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
        .map_err(|error| AppError::backend(format!("rpc `{method}` request failed: {error}")))?;

    let body: Value = response.into_json().map_err(|error| {
        AppError::backend(format!("failed to decode rpc `{method}` response: {error}"))
    })?;

    if let Some(error) = body.get("error") {
        return Err(AppError::backend(format!(
            "rpc `{method}` returned error: {error}"
        )));
    }

    body.get("result")
        .cloned()
        .ok_or_else(|| AppError::backend(format!("rpc `{method}` response is missing result")))
}

fn decode_hex_bytes(value: &str) -> AppResult<Vec<u8>> {
    let stripped = value.trim_start_matches("0x");

    if stripped.is_empty() {
        return Ok(Vec::new());
    }

    if stripped.len() % 2 != 0 {
        return Err(AppError::validation(format!(
            "hex payload must have even length, got `{value}`"
        )));
    }

    let mut output = Vec::with_capacity(stripped.len() / 2);
    let bytes = stripped.as_bytes();

    for index in (0..bytes.len()).step_by(2) {
        let chunk = std::str::from_utf8(&bytes[index..index + 2]).map_err(|e| {
            AppError::validation(format!("hex payload contains invalid utf-8: {e}"))
        })?;
        let byte = u8::from_str_radix(chunk, 16).map_err(|e| {
            AppError::validation(format!("failed to parse hex byte `{chunk}`: {e}"))
        })?;
        output.push(byte);
    }

    Ok(output)
}

fn parse_hex_u64(value: &str) -> AppResult<u64> {
    let stripped = value.trim_start_matches("0x");
    u64::from_str_radix(stripped, 16).map_err(|e| {
        AppError::validation(format!("failed to parse hex u64 `{value}`: {e}"))
    })
}

fn parse_hex_u256_str(value: &str) -> AppResult<String> {
    let stripped = value.trim_start_matches("0x");
    if stripped.is_empty() {
        return Ok("0".to_string());
    }
    // Pad to even length
    let padded;
    let s = if stripped.len() % 2 != 0 {
        padded = format!("0{stripped}");
        padded.as_str()
    } else {
        stripped
    };

    // Decode bytes
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let b = u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| AppError::validation(format!("hex parse error in `{value}`: {e}")))?;
        bytes.push(b);
    }

    // Convert big-endian bytes to decimal via u128 (sufficient for ETH values)
    let mut acc: u128 = 0;
    for b in &bytes {
        acc = acc
            .checked_mul(256)
            .and_then(|v| v.checked_add(*b as u128))
            .unwrap_or(u128::MAX);
    }
    Ok(acc.to_string())
}

// ---------------------------------------------------------------------------
// V2 swap ABI decoding
// ---------------------------------------------------------------------------

/// Decoded fields from a Uniswap V2 router swap call.
#[derive(Debug)]
pub struct DecodedV2Swap {
    /// The method name (e.g. "swapExactTokensForTokens").
    pub method: &'static str,
    /// First token in the path (token sold / ETH wrapper).
    pub token_in: Address,
    /// Last token in the path (token bought / ETH wrapper).
    pub token_out: Address,
    /// Full path of addresses.
    pub path: Vec<Address>,
    /// `amountIn` from calldata (0 for ETH-in variants).
    pub amount_in: U256,
    /// `amountOutMin` / `amountOut` depending on direction.
    pub amount_out_min: U256,
    /// True when the swap accepts ETH as input (swapExactETHForTokens*).
    pub eth_in: bool,
    /// True when the swap outputs ETH (swap*ForExactETH / swapExactTokensForETH*).
    pub eth_out: bool,
}

/// Known Uniswap V2 router selectors.
const SEL_SWAP_EXACT_TOKENS_FOR_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
const SEL_SWAP_TOKENS_FOR_EXACT_TOKENS: [u8; 4] = [0x88, 0x03, 0xdb, 0xee];
const SEL_SWAP_EXACT_ETH_FOR_TOKENS: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];
const SEL_SWAP_TOKENS_FOR_EXACT_ETH: [u8; 4] = [0x4a, 0x25, 0xd9, 0x4a];
const SEL_SWAP_EXACT_TOKENS_FOR_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];
const SEL_SWAP_ETH_FOR_EXACT_TOKENS: [u8; 4] = [0xfb, 0x3b, 0xdb, 0x41];

/// Attempt to decode `calldata` as one of the standard Uniswap V2 swap calls.
/// Returns `Ok(None)` when the selector is not recognised.
pub fn decode_v2_swap(calldata: &[u8]) -> AppResult<Option<DecodedV2Swap>> {
    if calldata.len() < 4 {
        return Ok(None);
    }

    let sel: [u8; 4] = calldata[..4].try_into().unwrap();
    let body = &calldata[4..];

    match sel {
        // swapExactTokensForTokens(amountIn, amountOutMin, path[], to, deadline)
        SEL_SWAP_EXACT_TOKENS_FOR_TOKENS => {
            let (amount_in, amount_out_min, path) = decode_amount_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapExactTokensForTokens",
                token_in,
                token_out,
                path,
                amount_in,
                amount_out_min,
                eth_in: false,
                eth_out: false,
            }))
        }
        // swapTokensForExactTokens(amountOut, amountInMax, path[], to, deadline)
        SEL_SWAP_TOKENS_FOR_EXACT_TOKENS => {
            let (amount_out, amount_in_max, path) = decode_amount_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapTokensForExactTokens",
                token_in,
                token_out,
                path,
                amount_in: amount_in_max,
                amount_out_min: amount_out,
                eth_in: false,
                eth_out: false,
            }))
        }
        // swapExactETHForTokens(amountOutMin, path[], to, deadline)  — no amountIn in calldata
        SEL_SWAP_EXACT_ETH_FOR_TOKENS => {
            let (amount_out_min, path) = decode_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapExactETHForTokens",
                token_in,
                token_out,
                path,
                amount_in: U256::ZERO,
                amount_out_min,
                eth_in: true,
                eth_out: false,
            }))
        }
        // swapTokensForExactETH(amountOut, amountInMax, path[], to, deadline)
        SEL_SWAP_TOKENS_FOR_EXACT_ETH => {
            let (amount_out, amount_in_max, path) = decode_amount_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapTokensForExactETH",
                token_in,
                token_out,
                path,
                amount_in: amount_in_max,
                amount_out_min: amount_out,
                eth_in: false,
                eth_out: true,
            }))
        }
        // swapExactTokensForETH(amountIn, amountOutMin, path[], to, deadline)
        SEL_SWAP_EXACT_TOKENS_FOR_ETH => {
            let (amount_in, amount_out_min, path) = decode_amount_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapExactTokensForETH",
                token_in,
                token_out,
                path,
                amount_in,
                amount_out_min,
                eth_in: false,
                eth_out: true,
            }))
        }
        // swapETHForExactTokens(amountOut, path[], to, deadline)
        SEL_SWAP_ETH_FOR_EXACT_TOKENS => {
            let (amount_out, path) = decode_amount_path(body)?;
            let (token_in, token_out) = endpoints(&path)?;
            Ok(Some(DecodedV2Swap {
                method: "swapETHForExactTokens",
                token_in,
                token_out,
                path,
                amount_in: U256::ZERO,
                amount_out_min: amount_out,
                eth_in: true,
                eth_out: false,
            }))
        }
        _ => Ok(None),
    }
}

// -- ABI helpers -------------------------------------------------------------

fn read_u256(data: &[u8], slot: usize) -> AppResult<U256> {
    let start = slot * 32;
    let end = start + 32;
    if data.len() < end {
        return Err(AppError::validation(format!(
            "ABI decode: data too short for slot {slot} (need {end}, got {})",
            data.len()
        )));
    }
    let bytes: [u8; 32] = data[start..end].try_into().unwrap();
    Ok(U256::from_be_bytes(bytes))
}

/// Decode `(uint256, uint256, address[], ...)` — first two scalars then a dynamic array.
fn decode_amount_amount_path(body: &[u8]) -> AppResult<(U256, U256, Vec<Address>)> {
    let a = read_u256(body, 0)?;
    let b = read_u256(body, 1)?;
    // slot 2 holds the offset to the path array (in bytes from start of body)
    let path_offset = usize::try_from(read_u256(body, 2)?)
        .map_err(|_| AppError::validation("ABI decode: path offset overflow"))?;
    let path = decode_address_array(body, path_offset)?;
    Ok((a, b, path))
}

/// Decode `(uint256, address[], ...)` — one scalar then a dynamic array.
fn decode_amount_path(body: &[u8]) -> AppResult<(U256, Vec<Address>)> {
    let a = read_u256(body, 0)?;
    // slot 1 holds the offset to the path array
    let path_offset = usize::try_from(read_u256(body, 1)?)
        .map_err(|_| AppError::validation("ABI decode: path offset overflow"))?;
    let path = decode_address_array(body, path_offset)?;
    Ok((a, path))
}

fn decode_address_array(body: &[u8], offset: usize) -> AppResult<Vec<Address>> {
    if body.len() < offset + 32 {
        return Err(AppError::validation("ABI decode: path array length out of bounds"));
    }
    let len_bytes: [u8; 32] = body[offset..offset + 32].try_into().unwrap();
    let len = usize::try_from(U256::from_be_bytes(len_bytes))
        .map_err(|_| AppError::validation("ABI decode: path length overflow"))?;

    let mut path = Vec::with_capacity(len);
    for i in 0..len {
        let elem_offset = offset + 32 + i * 32;
        if body.len() < elem_offset + 32 {
            return Err(AppError::validation(format!(
                "ABI decode: path element {i} out of bounds"
            )));
        }
        let bytes: [u8; 20] = body[elem_offset + 12..elem_offset + 32].try_into().unwrap();
        path.push(Address::from(bytes));
    }
    Ok(path)
}

fn endpoints(path: &[Address]) -> AppResult<(Address, Address)> {
    if path.len() < 2 {
        return Err(AppError::validation(format!(
            "ABI decode: swap path must have at least 2 addresses, got {}",
            path.len()
        )));
    }
    Ok((path[0], path[path.len() - 1]))
}
