use serde::{Deserialize, Serialize};

use crate::types::{SwapExecutionResult, SwapStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BlockExecutionResult {
    pub block_number: u64,
    #[serde(default)]
    pub swaps: Vec<SwapExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunSummary {
    pub processed_blocks: u64,
    pub total_swaps: u64,
    pub successes: u64,
    pub reverts: u64,
    pub skipped: u64,
    pub total_gas_used: u64,
    /// Sum of all `fee_amount` values across successful swaps (decimal string).
    pub total_fees: String,
    /// Sum of all `profit` values across successful swaps (decimal string).
    /// Since profit = fee_amount (DEX revenue), this equals `total_fees`.
    pub total_profit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunReport {
    #[serde(default)]
    pub blocks: Vec<BlockExecutionResult>,
    pub summary: RunSummary,
}

impl BlockExecutionResult {
    pub fn success_count(&self) -> u64 {
        self.swaps
            .iter()
            .filter(|swap| matches!(swap.status, SwapStatus::Success))
            .count() as u64
    }

    pub fn revert_count(&self) -> u64 {
        self.swaps
            .iter()
            .filter(|swap| matches!(swap.status, SwapStatus::Revert))
            .count() as u64
    }
}

impl RunSummary {
    pub fn record_block(&mut self, block: &BlockExecutionResult) {
        self.processed_blocks += 1;
        self.total_swaps += block.swaps.len() as u64;

        for swap in &block.swaps {
            match swap.status {
                SwapStatus::Success => self.successes += 1,
                SwapStatus::Revert => self.reverts += 1,
                SwapStatus::Skipped => self.skipped += 1,
            }

            self.total_gas_used += swap.gas_used.unwrap_or_default();

            if let Some(fee) = &swap.fee_amount {
                self.total_fees = add_decimal_strings(&self.total_fees, fee);
            }
            if let Some(profit) = &swap.profit {
                self.total_profit = add_decimal_strings(&self.total_profit, profit);
            }
        }
    }
}

impl RunReport {
    pub fn push_block(&mut self, block: BlockExecutionResult) {
        self.summary.record_block(&block);
        self.blocks.push(block);
    }
}

// ---------------------------------------------------------------------------
// Decimal string arithmetic helpers
// ---------------------------------------------------------------------------

/// Add two non-negative decimal strings. Falls back to "0" on parse error.
pub fn add_decimal_strings(a: &str, b: &str) -> String {
    let av: u128 = a.trim().parse().unwrap_or(0);
    let bv: u128 = b.trim().parse().unwrap_or(0);
    av.saturating_add(bv).to_string()
}

