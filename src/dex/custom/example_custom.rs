use crate::{
    backend::evm::{EvmBackend, TxRequest},
    config::DexConfig,
    dex::traits::DexSwapAdapter,
    error::{AppError, AppResult},
    types::{SwapExecutionResult, SwapRequest, SwapStatus},
};

#[derive(Debug, Clone)]
pub struct ExampleCustomAdapter {
    config: DexConfig,
}

impl ExampleCustomAdapter {
    pub fn new(config: DexConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &DexConfig {
        &self.config
    }

    fn encode_custom_swap(&self, swap: &SwapRequest) -> Vec<u8> {
        format!(
            "custom_swap|router={}|sender={}|pair={}->{}|amount_in={}|min_out={}",
            self.config.contracts.router,
            swap.sender,
            swap.token_in,
            swap.token_out,
            swap.amount_in,
            swap.min_amount_out
        )
        .into_bytes()
    }

    fn normalize_result(
        &self,
        swap: &SwapRequest,
        swap_index: usize,
        success: bool,
        gas_used: Option<u64>,
        payload: &[u8],
        fallback_error: Option<String>,
    ) -> SwapExecutionResult {
        if success {
            let decoded_amount_out = decode_non_empty_utf8(payload)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| swap.min_amount_out.clone());

            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Success,
                amount_out: Some(decoded_amount_out),
                gas_used,
                error: None,
            }
        } else {
            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Revert,
                amount_out: None,
                gas_used,
                error: fallback_error.or_else(|| decode_non_empty_utf8(payload)),
            }
        }
    }
}

impl DexSwapAdapter for ExampleCustomAdapter {
    fn from_config(config: DexConfig) -> AppResult<Self> {
        config.validate()?;

        if config.adapter != "custom.example" {
            return Err(AppError::validation(format!(
                "ExampleCustomAdapter requires `custom.example`, got `{}`",
                config.adapter
            )));
        }

        Ok(Self::new(config))
    }

    fn validate_swap(&self, swap: &SwapRequest) -> AppResult<()> {
        swap.validate()?;

        if swap.token_in.eq_ignore_ascii_case(&swap.token_out) {
            return Err(AppError::validation(
                "custom swap token_in and token_out must be different",
            ));
        }

        if swap.amount_in == "0" {
            return Err(AppError::validation(
                "custom swap requires amount_in greater than zero",
            ));
        }

        Ok(())
    }

    fn execute_swap<B: EvmBackend>(
        &self,
        backend: &mut B,
        swap: &SwapRequest,
        swap_index: usize,
    ) -> AppResult<SwapExecutionResult> {
        self.validate_swap(swap)?;

        let request = TxRequest {
            to: self.config.contracts.router.clone(),
            data: self.encode_custom_swap(swap),
            value: 0,
        };

        match backend.send_tx(request) {
            Ok(response) => Ok(self.normalize_result(
                swap,
                swap_index,
                response.success,
                Some(response.gas_used),
                &response.output,
                (!response.success).then(|| "custom swap reverted".to_string()),
            )),
            Err(error) => Ok(self.normalize_result(
                swap,
                swap_index,
                false,
                None,
                &[],
                Some(error.to_string()),
            )),
        }
    }
}

fn decode_non_empty_utf8(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let trimmed = text.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
