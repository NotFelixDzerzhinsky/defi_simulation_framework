use crate::{
    backend::evm::{EvmBackend, TxRequest},
    config::DexConfig,
    dex::traits::{compute_effective_amount_in, DexSwapAdapter},
    error::{AppError, AppResult},
    types::{SwapExecutionResult, SwapRequest, SwapStatus},
};

#[derive(Debug, Clone)]
pub struct ExampleCustomAdapter {
    config: DexConfig,
    fee_bps: u64,
}

impl ExampleCustomAdapter {
    pub fn new(config: DexConfig) -> Self {
        let fee_bps = config.fee_bps.unwrap_or(0);
        Self { config, fee_bps }
    }

    pub fn config(&self) -> &DexConfig {
        &self.config
    }

    fn encode_custom_swap(&self, swap: &SwapRequest, effective_amount_in: &str) -> Vec<u8> {
        format!(
            "custom_swap|router={}|sender={}|pair={}->{}|amount_in={}|min_out={}",
            self.config.contracts.router,
            swap.sender,
            swap.token_in,
            swap.token_out,
            effective_amount_in,
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
        fee_amount: String,
    ) -> SwapExecutionResult {
        if success {
            let amount_out = Some(
                decode_non_empty_utf8(payload)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| swap.min_amount_out.clone()),
            );
            // Profit for the DEX = fee collected from the trader.
            let profit = if fee_amount == "0" {
                None
            } else {
                Some(fee_amount.clone())
            };
            let fee_opt = if fee_amount == "0" {
                None
            } else {
                Some(fee_amount)
            };
            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Success,
                amount_out,
                gas_used,
                error: None,
                fee_amount: fee_opt,
                profit,
            }
        } else {
            SwapExecutionResult {
                block_number: swap.block_number,
                swap_index,
                status: SwapStatus::Revert,
                amount_out: None,
                gas_used,
                error: fallback_error.or_else(|| decode_non_empty_utf8(payload)),
                fee_amount: None,
                profit: None,
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

    fn fee_bps(&self) -> u64 {
        self.fee_bps
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

        // Deduct fee from amount_in BEFORE sending to the router.
        let (effective_amount_in, fee_amount) =
            compute_effective_amount_in(&swap.amount_in, self.fee_bps).ok_or_else(|| {
                AppError::validation(format!(
                    "cannot parse amount_in `{}` for fee computation",
                    swap.amount_in
                ))
            })?;

        let request = TxRequest {
            to: self.config.contracts.router.clone(),
            data: self.encode_custom_swap(swap, &effective_amount_in),
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
                fee_amount,
            )),
            Err(error) => Ok(self.normalize_result(
                swap,
                swap_index,
                false,
                None,
                &[],
                Some(error.to_string()),
                fee_amount,
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
