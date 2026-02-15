use crate::{
    backend::evm::EvmBackend, config::ExecutionConfig, dex::traits::DexSwapAdapter,
    engine::executor::SwapExecutor, error::AppResult, history::source::SwapInputSource,
    types::RunReport,
};

#[derive(Debug, Default)]
pub struct BlockLoop;

impl BlockLoop {
    pub fn run<B, A, S>(
        backend: &mut B,
        adapter: &A,
        source: &mut S,
        execution: &ExecutionConfig,
    ) -> AppResult<RunReport>
    where
        B: EvmBackend,
        A: DexSwapAdapter,
        S: SwapInputSource,
    {
        let mut report = RunReport::default();

        while let Some(block) = source.next_block()? {
            let block_result = SwapExecutor::execute_block(
                backend,
                adapter,
                &block,
                execution.continue_on_revert,
            )?;

            if execution.mine_after_block {
                backend.mine_block()?;
            }

            report.push_block(block_result);
        }

        Ok(report)
    }
}
