use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};

use crate::{
    backend::evm::{CallRequest, EvmBackend, TxRequest},
    config::DexConfig,
    dex::traits::{compute_effective_amount_in, DexSwapAdapter},
    error::{AppError, AppResult},
    types::{SwapExecutionResult, SwapRequest, SwapStatus},
};

const SIM_DEADLINE: u64 = u32::MAX as u64;

#[derive(Debug, Clone)]
pub struct UniswapV2Adapter {
    config: DexConfig,
    supported_tokens: BTreeSet<String>,
    fee_bps: u64,
}

/// Result of encoding a swap — calldata + ETH value to attach.
struct EncodedSwap {
    calldata: Vec<u8>,
    /// Non-zero for `swapExactETHForTokens` (msg.value = amountIn).
    value: u128,
}

impl UniswapV2Adapter {
    pub fn new(config: DexConfig) -> Self {
        let supported_tokens = config
            .tokens
            .iter()
            .map(|t| t.address.to_ascii_lowercase())
            .collect();

        let fee_bps = config.fee_bps.unwrap_or(0);

        Self {
            config,
            supported_tokens,
            fee_bps,
        }
    }

    pub fn config(&self) -> &DexConfig {
        &self.config
    }

    /// Encode the swap using the correct Router method based on metadata.
    ///
    /// `effective_amount_in` is the amount after fee deduction — this is what
    /// the router actually receives.
    ///
    /// - `eth_in=true`  → `swapExactETHForTokens` with `value = effective_amount_in`
    /// - `eth_out=true` → `swapExactTokensForETH` with `value = 0`
    /// - otherwise      → `swapExactTokensForTokens` with `value = 0`
    fn encode_swap(
        &self,
        swap: &SwapRequest,
        effective_amount_in: &str,
    ) -> AppResult<EncodedSwap> {
        let token_in = parse_address(&swap.token_in)?;
        let token_out = parse_address(&swap.token_out)?;
        let recipient = parse_address(&swap.sender)?;

        let amount_in = parse_u256(effective_amount_in)?;
        let amount_out_min = parse_u256(&swap.min_amount_out)?;

        let path = vec![token_in, token_out];
        let deadline = U256::from(SIM_DEADLINE);

        let eth_in = swap
            .metadata
            .get("eth_in")
            .map(|v| v == "true")
            .unwrap_or(false);
        let eth_out = swap
            .metadata
            .get("eth_out")
            .map(|v| v == "true")
            .unwrap_or(false);

        if eth_in {
            // swapExactETHForTokens — amountIn is sent as msg.value
            let calldata =
                encode_swap_exact_eth_for_tokens(amount_out_min, &path, recipient, deadline);
            let value: u128 = amount_in.try_into().unwrap_or(u128::MAX);
            Ok(EncodedSwap { calldata, value })
        } else if eth_out {
            // swapExactTokensForETH
            let calldata = encode_swap_exact_tokens_for_eth(
                amount_in,
                amount_out_min,
                &path,
                recipient,
                deadline,
            );
            Ok(EncodedSwap {
                calldata,
                value: 0,
            })
        } else {
            // swapExactTokensForTokens
            let calldata = encode_swap_exact_tokens_for_tokens(
                amount_in,
                amount_out_min,
                &path,
                recipient,
                deadline,
            );
            Ok(EncodedSwap {
                calldata,
                value: 0,
            })
        }
    }

    fn normalize_result(
        &self,
        swap: &SwapRequest,
        swap_index: usize,
        success: bool,
        gas_used: Option<u64>,
        output: &[u8],
        fallback_error: Option<String>,
        fee_amount: String,
    ) -> SwapExecutionResult {
        if success {
            let amount_out = decode_amounts_last(output);
            // Profit for the DEX = fee collected from the trader.
            let profit = if fee_amount == "0" {
                None
            } else {
                Some(fee_amount.clone())
            };
            let fee_opt = if fee_amount == "0" {
                None
            } else {
                Some(fee_amount)
            };
            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Success,
                amount_out,
                gas_used,
                error: None,
                fee_amount: fee_opt,
                profit,
            }
        } else {
            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Revert,
                amount_out: None,
                gas_used,
                error: fallback_error.or_else(|| decode_revert_reason(output)),
                fee_amount: None,
                profit: None,
            }
        }
    }
}

impl DexSwapAdapter for UniswapV2Adapter {
    fn from_config(config: DexConfig) -> AppResult<Self> {
        config.validate()?;

        if config.adapter != "builtin.uniswap_v2" {
            return Err(AppError::validation(format!(
                "UniswapV2Adapter requires adapter = \"builtin.uniswap_v2\", got `{}`",
                config.adapter
            )));
        }

        Ok(Self::new(config))
    }

    fn fee_bps(&self) -> u64 {
        self.fee_bps
    }

    fn validate_swap(&self, swap: &SwapRequest) -> AppResult<()> {
        swap.validate()?;

        if swap.token_in.eq_ignore_ascii_case(&swap.token_out) {
            return Err(AppError::validation(
                "swap token_in and token_out must be different",
            ));
        }

        if !self.supported_tokens.is_empty() {
            let token_in = swap.token_in.to_ascii_lowercase();
            let token_out = swap.token_out.to_ascii_lowercase();

            if !self.supported_tokens.contains(&token_in) {
                return Err(AppError::validation(format!(
                    "token_in `{}` is not listed in dex config tokens",
                    swap.token_in
                )));
            }

            if !self.supported_tokens.contains(&token_out) {
                return Err(AppError::validation(format!(
                    "token_out `{}` is not listed in dex config tokens",
                    swap.token_out
                )));
            }
        }

        // Validate that addresses are parseable
        parse_address(&swap.token_in).map_err(|_| {
            AppError::validation(format!(
                "token_in `{}` is not a valid Ethereum address",
                swap.token_in
            ))
        })?;
        parse_address(&swap.token_out).map_err(|_| {
            AppError::validation(format!(
                "token_out `{}` is not a valid Ethereum address",
                swap.token_out
            ))
        })?;
        parse_u256(&swap.amount_in).map_err(|_| {
            AppError::validation(format!(
                "amount_in `{}` is not a valid uint256",
                swap.amount_in
            ))
        })?;
        parse_u256(&swap.min_amount_out).map_err(|_| {
            AppError::validation(format!(
                "min_amount_out `{}` is not a valid uint256",
                swap.min_amount_out
            ))
        })?;

        Ok(())
    }

    fn execute_swap<B: EvmBackend>(
        &self,
        backend: &mut B,
        swap: &SwapRequest,
        swap_index: usize,
    ) -> AppResult<SwapExecutionResult> {
        self.validate_swap(swap)?;

        // Deduct fee from amount_in BEFORE sending to the router.
        let (effective_amount_in, fee_amount) =
            compute_effective_amount_in(&swap.amount_in, self.fee_bps).ok_or_else(|| {
                AppError::validation(format!(
                    "cannot parse amount_in `{}` for fee computation",
                    swap.amount_in
                ))
            })?;

        let encoded = self.encode_swap(swap, &effective_amount_in)?;

        // First, do an eth_call to get the return data (amounts[]).
        // eth_sendTransaction receipts do not include return data.
        let call_output = backend
            .call(CallRequest {
                to: self.config.contracts.router.clone(),
                data: encoded.calldata.clone(),
                value: Some(encoded.value),
            })
            .ok();

        let tx_request = TxRequest {
            to: self.config.contracts.router.clone(),
            data: encoded.calldata,
            value: encoded.value,
        };

        match backend.send_tx(tx_request) {
            Ok(response) => {
                // Use call output for decoding amounts if the tx succeeded.
                let output = if response.success {
                    call_output
                        .as_ref()
                        .map(|r| r.return_data.as_slice())
                        .unwrap_or(&response.output)
                } else {
                    &response.output
                };
                Ok(self.normalize_result(
                    swap,
                    swap_index,
                    response.success,
                    Some(response.gas_used),
                    output,
                    (!response.success).then(|| "swap reverted".to_string()),
                    fee_amount,
                ))
            }
            Err(error) => Ok(self.normalize_result(
                swap,
                swap_index,
                false,
                None,
                &[],
                Some(error.to_string()),
                fee_amount,
            )),
        }
    }
}


fn parse_address(s: &str) -> AppResult<Address> {
    s.parse::<Address>()
        .map_err(|e| AppError::validation(format!("invalid address `{s}`: {e}")))
}

fn parse_u256(s: &str) -> AppResult<U256> {
    U256::from_str_radix(s, 10)
        .map_err(|e| AppError::validation(format!("invalid uint256 `{s}`: {e}")))
}

/// Decode the last element of a `uint256[]` returned by Uniswap V2 swap methods.
/// The return value is `amounts[]` where the last element is the output amount.
fn decode_amounts_last(data: &[u8]) -> Option<String> {
    // ABI: offset (32) | length (32) | elements...
    if data.len() < 64 {
        return None;
    }

    let len_bytes: [u8; 32] = data[32..64].try_into().ok()?;
    let len = U256::from_be_bytes(len_bytes);
    let count: usize = len.try_into().ok()?;

    if count == 0 {
        return None;
    }

    let last_start = 64 + (count - 1) * 32;
    let last_end = last_start + 32;

    if data.len() < last_end {
        return None;
    }

    let word: [u8; 32] = data[last_start..last_end].try_into().ok()?;
    let value = U256::from_be_bytes(word);
    Some(value.to_string())
}

/// Try to decode a Solidity `Error(string)` revert reason.
fn decode_revert_reason(data: &[u8]) -> Option<String> {
    // selector 0x08c379a0 + offset(32) + length(32) + string bytes
    if data.len() < 68 {
        return None;
    }

    let selector: [u8; 4] = data[..4].try_into().ok()?;
    if selector != [0x08, 0xc3, 0x79, 0xa0] {
        return None;
    }

    let len_bytes: [u8; 32] = data[36..68].try_into().ok()?;
    let len = U256::from_be_bytes(len_bytes);
    let str_len: usize = len.try_into().ok()?;

    let str_end = 68 + str_len;
    if data.len() < str_end {
        return None;
    }

    String::from_utf8(data[68..str_end].to_vec()).ok()
}

// ---------------------------------------------------------------------------
// ABI encoding helpers
// ---------------------------------------------------------------------------

/// ABI-encodes a call to `swapExactTokensForTokens(uint256,uint256,address[],address,uint256)`.
/// Selector: 0x38ed1739
fn encode_swap_exact_tokens_for_tokens(
    amount_in: U256,
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Vec<u8> {
    let selector: [u8; 4] = [0x38, 0xed, 0x17, 0x39];

    // The dynamic `path` array starts after the 5 head slots (5 * 32 = 160 bytes).
    let path_offset = U256::from(160u64);

    let mut out = Vec::with_capacity(4 + 32 * (5 + 1 + path.len()));
    out.extend_from_slice(&selector);

    // head: amountIn, amountOutMin, offset-to-path, to, deadline
    out.extend_from_slice(&amount_in.to_be_bytes::<32>());
    out.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
    out.extend_from_slice(&path_offset.to_be_bytes::<32>());

    // `to` address — zero-padded to 32 bytes (left-padded)
    let mut to_word = [0u8; 32];
    to_word[12..].copy_from_slice(to.as_slice());
    out.extend_from_slice(&to_word);

    out.extend_from_slice(&deadline.to_be_bytes::<32>());

    // tail: path array length + elements
    let path_len = U256::from(path.len() as u64);
    out.extend_from_slice(&path_len.to_be_bytes::<32>());
    for addr in path {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(addr.as_slice());
        out.extend_from_slice(&word);
    }

    out
}

/// ABI-encodes a call to `swapExactETHForTokens(uint256,address[],address,uint256)`.
/// Selector: 0x7ff36ab5
/// Note: amountIn is sent as msg.value, NOT in calldata.
fn encode_swap_exact_eth_for_tokens(
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Vec<u8> {
    let selector: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];

    // head: amountOutMin, offset-to-path, to, deadline  (4 head slots → offset = 128)
    let path_offset = U256::from(128u64);

    let mut out = Vec::with_capacity(4 + 32 * (4 + 1 + path.len()));
    out.extend_from_slice(&selector);

    out.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
    out.extend_from_slice(&path_offset.to_be_bytes::<32>());

    let mut to_word = [0u8; 32];
    to_word[12..].copy_from_slice(to.as_slice());
    out.extend_from_slice(&to_word);

    out.extend_from_slice(&deadline.to_be_bytes::<32>());

    // tail: path array
    let path_len = U256::from(path.len() as u64);
    out.extend_from_slice(&path_len.to_be_bytes::<32>());
    for addr in path {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(addr.as_slice());
        out.extend_from_slice(&word);
    }

    out
}

/// ABI-encodes a call to `swapExactTokensForETH(uint256,uint256,address[],address,uint256)`.
/// Selector: 0x18cbafe5
fn encode_swap_exact_tokens_for_eth(
    amount_in: U256,
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Vec<u8> {
    let selector: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];

    // Same layout as swapExactTokensForTokens: 5 head slots → offset = 160
    let path_offset = U256::from(160u64);

    let mut out = Vec::with_capacity(4 + 32 * (5 + 1 + path.len()));
    out.extend_from_slice(&selector);

    out.extend_from_slice(&amount_in.to_be_bytes::<32>());
    out.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
    out.extend_from_slice(&path_offset.to_be_bytes::<32>());

    let mut to_word = [0u8; 32];
    to_word[12..].copy_from_slice(to.as_slice());
    out.extend_from_slice(&to_word);

    out.extend_from_slice(&deadline.to_be_bytes::<32>());

    // tail: path array
    let path_len = U256::from(path.len() as u64);
    out.extend_from_slice(&path_len.to_be_bytes::<32>());
    for addr in path {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(addr.as_slice());
        out.extend_from_slice(&word);
    }

    out
}
