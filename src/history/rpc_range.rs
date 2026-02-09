use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

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

impl RpcRangeSource {
    pub fn from_config(config: &HistorySourceConfig) -> AppResult<Self> {
        match config {
            HistorySourceConfig::RpcRange {
                rpc_url,
                start_block,
                end_block,
                contract_address,
                method,
            } => Self::from_fixture_path(
                rpc_url,
                *start_block,
                *end_block,
                contract_address,
                method.as_deref(),
            ),
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

        Self::from_transactions(
            payload.transactions,
            start_block,
            end_block,
            contract_address,
            method,
        )
    }

    fn from_transactions(
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

fn resolve_fixture_path(rpc_url: &str) -> AppResult<PathBuf> {
    if rpc_url.starts_with("http://") || rpc_url.starts_with("https://") {
        return Err(AppError::validation(
            "live RPC fetching is not implemented yet; use a local fixture path for rpc_range tests",
        ));
    }

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

fn validate_blocks(blocks: &[BlockSwaps]) -> AppResult<()> {
    for block in blocks {
        block.validate()?;
    }

    Ok(())
}

#[allow(dead_code)]
fn _path_debug(path: &Path) -> PathBuf {
    path.to_path_buf()
}
