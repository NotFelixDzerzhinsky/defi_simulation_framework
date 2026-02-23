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
        }
    }
}

impl RunReport {
    pub fn push_block(&mut self, block: BlockExecutionResult) {
        self.summary.record_block(&block);
        self.blocks.push(block);
    }
}
