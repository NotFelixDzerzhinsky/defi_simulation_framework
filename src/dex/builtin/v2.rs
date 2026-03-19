use std::collections::BTreeSet;

use crate::{
    backend::evm::{EvmBackend, TxRequest},
    config::DexConfig,
    dex::traits::{compute_effective_amount_in, DexSwapAdapter},
    error::{AppError, AppResult},
    types::{SwapExecutionResult, SwapRequest, SwapStatus},
};

#[derive(Debug, Clone)]
pub struct V2Adapter {
    config: DexConfig,
    supported_tokens: BTreeSet<String>,
    fee_bps: u64,
}

impl V2Adapter {
    pub fn new(config: DexConfig) -> Self {
        let supported_tokens = config
            .tokens
            .iter()
            .map(|token| token.address.to_ascii_lowercase())
            .collect();

        let fee_bps = config.fee_bps.unwrap_or(0);

        Self {
            config,
            supported_tokens,
            fee_bps,
        }
    }

    pub fn config(&self) -> &DexConfig {
        &self.config
    }

    fn encode_swap_call(&self, swap: &SwapRequest, effective_amount_in: &str) -> Vec<u8> {
        format!(
            "swap_exact_input|sender={}|token_in={}|token_out={}|amount_in={}|min_amount_out={}",
            swap.sender, swap.token_in, swap.token_out, effective_amount_in, swap.min_amount_out
        )
        .into_bytes()
    }

    fn normalize_execution(
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
            let amount_out = decode_non_empty_utf8(payload);
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

impl DexSwapAdapter for V2Adapter {
    fn from_config(config: DexConfig) -> AppResult<Self> {
        config.validate()?;

        if config.adapter != "builtin.v2" {
            return Err(AppError::validation(format!(
                "V2Adapter requires `builtin.v2`, got `{}`",
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
                "swap token_in and token_out must be different",
            ));
        }

        if !self.supported_tokens.is_empty() {
            let token_in = swap.token_in.to_ascii_lowercase();
            let token_out = swap.token_out.to_ascii_lowercase();

            if !self.supported_tokens.contains(&token_in) {
                return Err(AppError::validation(format!(
                    "token_in `{}` is not listed in dex config",
                    swap.token_in
                )));
            }

            if !self.supported_tokens.contains(&token_out) {
                return Err(AppError::validation(format!(
                    "token_out `{}` is not listed in dex config",
                    swap.token_out
                )));
            }
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

        let tx_request = TxRequest {
            to: self.config.contracts.router.clone(),
            data: self.encode_swap_call(swap, &effective_amount_in),
            value: 0,
        };

        match backend.send_tx(tx_request) {
            Ok(response) => Ok(self.normalize_execution(
                swap,
                swap_index,
                response.success,
                Some(response.gas_used),
                &response.output,
                (!response.success).then(|| "swap reverted".to_string()),
                fee_amount,
            )),
            Err(error) => Ok(self.normalize_execution(
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
