use crate::{
    backend::evm::EvmBackend,
    config::DexConfig,
    dex::{
        builtin::v2::V2Adapter, custom::example_custom::ExampleCustomAdapter,
        traits::DexSwapAdapter,
    },
    error::{AppError, AppResult},
    types::{SwapExecutionResult, SwapRequest},
};

#[derive(Debug, Clone)]
pub enum RegisteredDexAdapter {
    BuiltinV2(V2Adapter),
    ExampleCustom(ExampleCustomAdapter),
}

pub fn builtin_adapter_names() -> &'static [&'static str] {
    &["builtin.v2"]
}

pub fn build_adapter(config: DexConfig) -> AppResult<RegisteredDexAdapter> {
    match config.adapter.as_str() {
        "builtin.v2" => Ok(RegisteredDexAdapter::BuiltinV2(V2Adapter::from_config(
            config,
        )?)),
        "custom.example" => Ok(RegisteredDexAdapter::ExampleCustom(
            ExampleCustomAdapter::from_config(config)?,
        )),
        other => Err(AppError::validation(format!(
            "unsupported dex adapter `{other}`"
        ))),
    }
}

impl RegisteredDexAdapter {
    pub fn adapter_name(&self) -> &'static str {
        match self {
            Self::BuiltinV2(_) => "builtin.v2",
            Self::ExampleCustom(_) => "custom.example",
        }
    }
}

impl DexSwapAdapter for RegisteredDexAdapter {
    fn from_config(config: DexConfig) -> AppResult<Self> {
        build_adapter(config)
    }

    fn validate_swap(&self, swap: &SwapRequest) -> AppResult<()> {
        match self {
            Self::BuiltinV2(adapter) => adapter.validate_swap(swap),
            Self::ExampleCustom(adapter) => adapter.validate_swap(swap),
        }
    }

    fn execute_swap<B: EvmBackend>(
        &self,
        backend: &mut B,
        swap: &SwapRequest,
        swap_index: usize,
    ) -> AppResult<SwapExecutionResult> {
        match self {
            Self::BuiltinV2(adapter) => adapter.execute_swap(backend, swap, swap_index),
            Self::ExampleCustom(adapter) => adapter.execute_swap(backend, swap, swap_index),
        }
    }
}
