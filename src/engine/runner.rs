use crate::{
    backend::evm::EvmBackend, config::ExecutionConfig, dex::traits::DexSwapAdapter,
    engine::block_loop::BlockLoop, error::AppResult, history::source::SwapInputSource,
    types::RunReport,
};

#[derive(Debug, Default)]
pub struct EngineRunner;

impl EngineRunner {
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
        BlockLoop::run(backend, adapter, source, execution)
    }
}
