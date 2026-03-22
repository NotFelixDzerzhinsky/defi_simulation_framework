use crate::{
    backend::evm::EvmBackend,
    config::DexConfig,
    error::AppResult,
    types::{SwapExecutionResult, SwapRequest},
};

pub trait DexSwapAdapter: Sized {
    fn from_config(config: DexConfig) -> AppResult<Self>;

    /// Fee charged by this DEX in basis points (1 bps = 0.01%).
    /// Returns 0 if no fee is configured.
    fn fee_bps(&self) -> u64;

    fn validate_swap(&self, swap: &SwapRequest) -> AppResult<()>;
    fn execute_swap<B: EvmBackend>(
        &self,
        backend: &mut B,
        swap: &SwapRequest,
        swap_index: usize,
    ) -> AppResult<SwapExecutionResult>;
}

// ---------------------------------------------------------------------------
// Fee / profit computation helpers (shared by all adapters)
// ---------------------------------------------------------------------------

/// Compute the effective amount sent to the router after deducting the DEX fee.
///
/// Returns `(effective_amount_in, fee_amount)` as decimal strings.
/// When `fee_bps` is 0 the full `amount_in` is returned and `fee_amount` is `"0"`.
/// Returns `None` only when `amount_in` cannot be parsed.
pub fn compute_effective_amount_in(amount_in: &str, fee_bps: u64) -> Option<(String, String)> {
    let amount: u128 = amount_in.trim().parse().ok()?;
    if fee_bps == 0 {
        return Some((amount.to_string(), "0".to_string()));
    }
    let fee = amount * fee_bps as u128 / 10_000;
    let effective = amount.saturating_sub(fee);
    Some((effective.to_string(), fee.to_string()))
}

