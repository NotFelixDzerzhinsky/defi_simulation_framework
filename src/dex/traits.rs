use crate::{
    backend::evm::EvmBackend,
    config::DexConfig,
    error::AppResult,
    types::{SwapExecutionResult, SwapRequest},
};

pub trait DexSwapAdapter: Sized {
    fn from_config(config: DexConfig) -> AppResult<Self>;
    fn validate_swap(&self, swap: &SwapRequest) -> AppResult<()>;
    fn execute_swap<B: EvmBackend>(
        &self,
        backend: &mut B,
        swap: &SwapRequest,
        swap_index: usize,
    ) -> AppResult<SwapExecutionResult>;
}
