use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    config::HistorySourceConfig,
    error::{AppError, AppResult},
    history::{normalize::group_swaps_by_block, source::SwapInputSource},
    types::{BlockSwaps, SwapRequest},
};

#[derive(Debug, Default)]
pub struct RpcRangeSource {
    blocks: VecDeque<BlockSwaps>,
}

#[derive(Debug, Deserialize)]
struct RpcFixturePayload {
    #[serde(default)]
    transactions: Vec<RpcFixtureTransaction>,
}

#[derive(Debug, Clone, Deserialize)]
struct RpcFixtureTransaction {
    pub block_number: u64,
    pub to: String,
    #[serde(default)]
    pub method: Option<String>,
    pub sender: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub min_amount_out: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub tx_index: Option<u64>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LiveRpcTransaction {
    hash: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    input: String,
    #[serde(rename = "blockNumber")]
    block_number: Option<String>,
    #[serde(rename = "transactionIndex")]
    tx_index: Option<String>,
}

impl RpcRangeSource {
    pub fn from_config(config: &HistorySourceConfig) -> AppResult<Self> {
        match config {
            HistorySourceConfig::RpcRange {
                rpc_url,
                start_block,
                end_block,
                contract_address,
                method,
            } => {
                if is_live_rpc_url(rpc_url) {
                    Self::from_live_rpc(
                        rpc_url,
                        *start_block,
                        *end_block,
                        contract_address,
                        method.as_deref(),
                    )
                } else {
                    Self::from_fixture_path(
                        rpc_url,
                        *start_block,
                        *end_block,
                        contract_address,
                        method.as_deref(),
                    )
                }
            }
            other => Err(AppError::validation(format!(
                "expected rpc_range history config, got `{}`",
                other.kind()
            ))),
        }
    }

    pub fn from_fixture_path(
        rpc_url: &str,
        start_block: u64,
        end_block: u64,
        contract_address: &str,
        method: Option<&str>,
    ) -> AppResult<Self> {
        let path = resolve_fixture_path(rpc_url)?;
        let raw = fs::read_to_string(&path).map_err(|source| AppError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let payload: RpcFixturePayload = serde_json::from_str(&raw).map_err(|source| {
            AppError::validation(format!(
                "failed to parse rpc fixture `{}`: {source}",
                path.display()
            ))
        })?;

        Self::from_fixture_transactions(
            payload.transactions,
            start_block,
            end_block,
            contract_address,
            method,
        )
    }

    pub fn from_live_rpc(
        rpc_url: &str,
        start_block: u64,
        end_block: u64,
        contract_address: &str,
        method: Option<&str>,
    ) -> AppResult<Self> {
        let transactions = fetch_live_transactions(rpc_url, start_block, end_block)?;
        Self::from_live_transactions(
            transactions,
            start_block,
            end_block,
            contract_address,
            method,
        )
    }

    fn from_fixture_transactions(
        transactions: Vec<RpcFixtureTransaction>,
        start_block: u64,
        end_block: u64,
        contract_address: &str,
        method: Option<&str>,
    ) -> AppResult<Self> {
        let contract_address = contract_address.to_ascii_lowercase();
        let expected_method = method.map(str::to_ascii_lowercase);

        let swaps = transactions
            .into_iter()
            .filter(|tx| tx.block_number >= start_block && tx.block_number <= end_block)
            .filter(|tx| tx.to.eq_ignore_ascii_case(&contract_address))
            .filter(|tx| match (&expected_method, &tx.method) {
                (Some(expected), Some(actual)) => actual.eq_ignore_ascii_case(expected),
                (Some(_), None) => false,
                (None, _) => true,
            })
            .map(|tx| SwapRequest {
                block_number: tx.block_number,
                sender: tx.sender,
                token_in: tx.token_in,
                token_out: tx.token_out,
                amount_in: tx.amount_in,
                min_amount_out: tx.min_amount_out,
                tx_hash: tx.tx_hash,
                tx_index: tx.tx_index,
                metadata: tx.metadata,
            })
            .collect::<Vec<_>>();

        Self::from_swaps(swaps)
    }

    fn from_live_transactions(
        transactions: Vec<LiveRpcTransaction>,
        start_block: u64,
        end_block: u64,
        contract_address: &str,
        method: Option<&str>,
    ) -> AppResult<Self> {
        let expected_contract = contract_address.to_ascii_lowercase();
        let expected_method = method.map(str::to_ascii_lowercase);
        let mut swaps = Vec::new();

        for tx in transactions {
            let Some(to) = tx.to.as_ref() else {
                continue;
            };

            if !to.eq_ignore_ascii_case(&expected_contract) {
                continue;
            }

            let block_number = parse_hex_u64_str(tx.block_number.as_deref().ok_or_else(|| {
                AppError::validation(format!(
                    "rpc transaction `{}` is missing blockNumber",
                    tx.hash
                ))
            })?)?;

            if block_number < start_block || block_number > end_block {
                continue;
            }

            let decoded = decode_live_transaction(&tx)?;

            if let Some(expected_method) = &expected_method {
                if !decoded.method.eq_ignore_ascii_case(expected_method) {
                    continue;
                }
            }

            let tx_index = tx
                .tx_index
                .as_deref()
                .map(parse_hex_u64_str)
                .transpose()
                .map_err(|error| {
                    AppError::validation(format!(
                        "failed to parse transactionIndex for `{}`: {error}",
                        tx.hash
                    ))
                })?;

            let mut metadata = BTreeMap::new();
            metadata.insert("history_source".to_string(), "rpc_range".to_string());
            metadata.insert("decoded_method".to_string(), decoded.method.clone());

            swaps.push(SwapRequest {
                block_number,
                sender: decoded.sender.or_else(|| tx.from.clone()).ok_or_else(|| {
                    AppError::validation(format!(
                        "rpc transaction `{}` is missing sender information",
                        tx.hash
                    ))
                })?,
                token_in: decoded.token_in,
                token_out: decoded.token_out,
                amount_in: decoded.amount_in,
                min_amount_out: decoded.min_amount_out,
                tx_hash: Some(tx.hash),
                tx_index,
                metadata,
            });
        }

        Self::from_swaps(swaps)
    }

    fn from_swaps(swaps: Vec<SwapRequest>) -> AppResult<Self> {
        for swap in &swaps {
            swap.validate()?;
        }

        let blocks = group_swaps_by_block(swaps);
        validate_blocks(&blocks)?;

        Ok(Self {
            blocks: VecDeque::from(blocks),
        })
    }

    pub fn remaining_blocks(&self) -> usize {
        self.blocks.len()
    }
}

impl SwapInputSource for RpcRangeSource {
    fn next_block(&mut self) -> AppResult<Option<BlockSwaps>> {
        Ok(self.blocks.pop_front())
    }
}

#[derive(Debug)]
struct DecodedRpcSwap {
    method: String,
    sender: Option<String>,
    token_in: String,
    token_out: String,
    amount_in: String,
    min_amount_out: String,
}

fn decode_live_transaction(tx: &LiveRpcTransaction) -> AppResult<DecodedRpcSwap> {
    let payload_bytes = decode_hex_bytes(&tx.input).map_err(|error| {
        AppError::validation(format!(
            "failed to decode tx input for `{}`: {error}",
            tx.hash
        ))
    })?;
    let payload = String::from_utf8(payload_bytes).map_err(|error| {
        AppError::validation(format!(
            "tx input for `{}` is not valid utf-8 payload: {error}",
            tx.hash
        ))
    })?;

    decode_payload(&payload).map_err(|error| {
        AppError::validation(format!(
            "failed to decode swap payload for tx `{}`: {error}",
            tx.hash
        ))
    })
}

fn decode_payload(payload: &str) -> AppResult<DecodedRpcSwap> {
    let mut parts = payload.split('|');
    let method = parts
        .next()
        .ok_or_else(|| AppError::validation("empty rpc payload"))?
        .trim()
        .to_string();

    if method.is_empty() {
        return Err(AppError::validation("rpc payload method must not be empty"));
    }

    let mut kv = BTreeMap::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (key, value) = trimmed.split_once('=').ok_or_else(|| {
            AppError::validation(format!("rpc payload segment `{trimmed}` is not key=value"))
        })?;
        kv.insert(key.trim().to_string(), value.trim().to_string());
    }

    match method.as_str() {
        "swap_exact_input" => Ok(DecodedRpcSwap {
            method,
            sender: kv.get("sender").cloned(),
            token_in: required_value(&kv, "token_in")?,
            token_out: required_value(&kv, "token_out")?,
            amount_in: required_value(&kv, "amount_in")?,
            min_amount_out: required_value(&kv, "min_amount_out")?,
        }),
        "custom_swap" => {
            let pair = required_value(&kv, "pair")?;
            let (token_in, token_out) = pair.split_once("->").ok_or_else(|| {
                AppError::validation(format!(
                    "custom_swap pair `{pair}` must use `token_in->token_out` format"
                ))
            })?;

            Ok(DecodedRpcSwap {
                method,
                sender: kv.get("sender").cloned(),
                token_in: token_in.to_string(),
                token_out: token_out.to_string(),
                amount_in: required_value(&kv, "amount_in")?,
                min_amount_out: required_value(&kv, "min_out")?,
            })
        }
        other => Err(AppError::validation(format!(
            "unsupported rpc payload method `{other}`"
        ))),
    }
}

fn required_value(values: &BTreeMap<String, String>, key: &str) -> AppResult<String> {
    values.get(key).cloned().ok_or_else(|| {
        AppError::validation(format!("rpc payload is missing required field `{key}`"))
    })
}

fn fetch_live_transactions(
    rpc_url: &str,
    start_block: u64,
    end_block: u64,
) -> AppResult<Vec<LiveRpcTransaction>> {
    let mut transactions = Vec::new();

    for block_number in start_block..=end_block {
        let block = rpc_request(
            rpc_url,
            "eth_getBlockByNumber",
            json!([format!("0x{block_number:x}"), true]),
        )?;

        if block.is_null() {
            continue;
        }

        let Some(block_transactions) = block.get("transactions").and_then(Value::as_array) else {
            return Err(AppError::backend(format!(
                "block {block_number} response is missing `transactions`"
            )));
        };

        for tx in block_transactions {
            let tx: LiveRpcTransaction = serde_json::from_value(tx.clone()).map_err(|error| {
                AppError::backend(format!(
                    "failed to decode transaction from block {block_number}: {error}"
                ))
            })?;
            transactions.push(tx);
        }
    }

    Ok(transactions)
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

fn resolve_fixture_path(rpc_url: &str) -> AppResult<PathBuf> {
    let path = rpc_url
        .strip_prefix("file://")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(rpc_url));

    if path.as_os_str().is_empty() {
        return Err(AppError::validation(
            "rpc_range fixture path must not be empty",
        ));
    }

    Ok(path)
}

fn is_live_rpc_url(rpc_url: &str) -> bool {
    rpc_url.starts_with("http://") || rpc_url.starts_with("https://")
}

fn validate_blocks(blocks: &[BlockSwaps]) -> AppResult<()> {
    for block in blocks {
        block.validate()?;
    }

    Ok(())
}

fn parse_hex_u64_str(value: &str) -> AppResult<u64> {
    let stripped = value.trim_start_matches("0x");
    u64::from_str_radix(stripped, 16).map_err(|error| {
        AppError::validation(format!("failed to parse hex value `{value}`: {error}"))
    })
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
        let chunk = std::str::from_utf8(&bytes[index..index + 2]).map_err(|error| {
            AppError::validation(format!("hex payload contains invalid utf-8: {error}"))
        })?;
        let byte = u8::from_str_radix(chunk, 16).map_err(|error| {
            AppError::validation(format!("failed to parse hex byte `{chunk}`: {error}"))
        })?;
        output.push(byte);
    }

    Ok(output)
}

#[allow(dead_code)]
fn _path_debug(path: &Path) -> PathBuf {
    path.to_path_buf()
}
