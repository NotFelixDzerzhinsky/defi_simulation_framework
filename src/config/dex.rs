use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DexConfig {
    pub name: String,
    pub adapter: String,
    pub contracts: DexContracts,
    /// Fee in basis points (1 bps = 0.01%). E.g. 30 = 0.3% (Uniswap V2 standard).
    /// Used to compute `fee_amount` per swap and accumulate `total_fees` in the run summary.
    #[serde(default)]
    pub fee_bps: Option<u64>,
    #[serde(default)]
    pub assets: DexAssets,
    #[serde(default)]
    pub tokens: Vec<TokenConfig>,
    #[serde(default)]
    pub protocol: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DexContracts {
    pub router: String,
    #[serde(default)]
    pub factory: Option<String>,
    #[serde(default)]
    pub quoter: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexAssets {
    #[serde(default)]
    pub artifacts_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenConfig {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
}

impl DexConfig {
    pub fn resolve_relative_paths(&mut self, base_dir: &std::path::Path) {
        if let Some(artifacts_dir) = &mut self.assets.artifacts_dir {
            if artifacts_dir.is_relative() {
                *artifacts_dir = base_dir.join(&*artifacts_dir);
            }
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        require_non_empty("dex.name", &self.name)?;
        require_non_empty("dex.adapter", &self.adapter)?;
        require_non_empty("dex.contracts.router", &self.contracts.router)?;

        if let Some(fee_bps) = self.fee_bps {
            if fee_bps > 10_000 {
                return Err(AppError::validation(format!(
                    "dex.fee_bps must be <= 10000 (100%), got {fee_bps}"
                )));
            }
        }

        if let Some(factory) = &self.contracts.factory {
            require_non_empty("dex.contracts.factory", factory)?;
        }

        if let Some(quoter) = &self.contracts.quoter {
            require_non_empty("dex.contracts.quoter", quoter)?;
        }

        if let Some(artifacts_dir) = &self.assets.artifacts_dir {
            require_path("dex.assets.artifacts_dir", artifacts_dir)?;
        }

        let mut symbols = std::collections::BTreeSet::new();
        let mut addresses = std::collections::BTreeSet::new();

        for token in &self.tokens {
            require_non_empty("dex.tokens[].symbol", &token.symbol)?;
            require_non_empty("dex.tokens[].address", &token.address)?;

            if !symbols.insert(token.symbol.to_ascii_uppercase()) {
                return Err(AppError::validation(format!(
                    "duplicate token symbol `{}` in dex.tokens",
                    token.symbol
                )));
            }

            if !addresses.insert(token.address.to_ascii_lowercase()) {
                return Err(AppError::validation(format!(
                    "duplicate token address `{}` in dex.tokens",
                    token.address
                )));
            }
        }

        Ok(())
    }
}

fn require_non_empty(field: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::validation(format!(
            "required field `{field}` must not be empty"
        )));
    }

    Ok(())
}

fn require_path(field: &str, value: &PathBuf) -> AppResult<()> {
    if value.as_os_str().is_empty() {
        return Err(AppError::validation(format!(
            "required field `{field}` must not be empty"
        )));
    }

    Ok(())
}
