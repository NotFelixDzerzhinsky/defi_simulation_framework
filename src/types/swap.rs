use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapRequest {
    pub block_number: u64,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapStatus {
    Success,
    Revert,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapExecutionResult {
    pub block_number: u64,
    pub swap_index: usize,
    pub status: SwapStatus,
    #[serde(default)]
    pub amount_out: Option<String>,
    #[serde(default)]
    pub gas_used: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

impl SwapRequest {
    pub fn validate(&self) -> AppResult<()> {
        require_non_empty("swap.sender", &self.sender)?;
        require_non_empty("swap.token_in", &self.token_in)?;
        require_non_empty("swap.token_out", &self.token_out)?;
        require_non_empty("swap.amount_in", &self.amount_in)?;
        require_non_empty("swap.min_amount_out", &self.min_amount_out)?;

        if let Some(tx_hash) = &self.tx_hash {
            require_non_empty("swap.tx_hash", tx_hash)?;
        }

        Ok(())
    }
}

impl SwapExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(self.status, SwapStatus::Success)
    }
}

fn require_non_empty(field: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::validation(format!(
            "required field `{field}` must not be empty"
        )));
    }

    Ok(())
}
