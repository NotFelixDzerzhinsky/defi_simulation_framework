use crate::{
    backend::evm::EvmBackend,
    dex::traits::DexSwapAdapter,
    error::AppResult,
    types::{BlockExecutionResult, BlockSwaps, SwapExecutionResult, SwapStatus},
};

#[derive(Debug, Default)]
pub struct SwapExecutor;

impl SwapExecutor {
    pub fn execute_block<B, A>(
        backend: &mut B,
        adapter: &A,
        block: &BlockSwaps,
        continue_on_revert: bool,
    ) -> AppResult<BlockExecutionResult>
    where
        B: EvmBackend,
        A: DexSwapAdapter,
    {
        block.validate()?;

        let mut results = Vec::with_capacity(block.swaps.len());
        let mut halt_after_revert = false;

        for (swap_index, swap) in block.swaps.iter().enumerate() {
            if halt_after_revert {
                results.push(SwapExecutionResult {
                    block_number: block.block_number,
                    swap_index,
                    status: SwapStatus::Skipped,
                    amount_out: None,
                    gas_used: None,
                    error: Some(
                        "skipped because a previous swap in the block reverted".to_string(),
                    ),
                });
                continue;
            }

            let result = adapter.execute_swap(backend, swap, swap_index)?;

            if matches!(result.status, SwapStatus::Revert) && !continue_on_revert {
                halt_after_revert = true;
            }

            results.push(result);
        }

        Ok(BlockExecutionResult {
            block_number: block.block_number,
            swaps: results,
        })
    }
}
