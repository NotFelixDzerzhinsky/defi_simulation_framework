use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    types::swap::SwapRequest,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BlockSwaps {
    pub block_number: u64,
    #[serde(default)]
    pub swaps: Vec<SwapRequest>,
}

impl BlockSwaps {
    pub fn validate(&self) -> AppResult<()> {
        for (index, swap) in self.swaps.iter().enumerate() {
            swap.validate()?;

            if swap.block_number != self.block_number {
                return Err(AppError::validation(format!(
                    "swap at index {index} belongs to block {} but container block is {}",
                    swap.block_number, self.block_number
                )));
            }
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.swaps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.swaps.is_empty()
    }
}
