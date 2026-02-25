use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub source: HistorySourceConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HistorySourceConfig {
    Jsonl {
        path: PathBuf,
    },
    RpcRange {
        rpc_url: String,
        start_block: u64,
        end_block: u64,
        contract_address: String,
        #[serde(default)]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionConfig {
    #[serde(default = "default_true")]
    pub mine_after_block: bool,
    #[serde(default)]
    pub continue_on_revert: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            mine_after_block: true,
            continue_on_revert: false,
        }
    }
}

impl HistoryConfig {
    pub fn resolve_relative_paths(&mut self, base_dir: &std::path::Path) {
        match &mut self.source {
            HistorySourceConfig::Jsonl { path } => {
                if path.is_relative() {
                    *path = base_dir.join(&*path);
                }
            }
            HistorySourceConfig::RpcRange { rpc_url, .. } => {
                if !rpc_url.starts_with("http://")
                    && !rpc_url.starts_with("https://")
                    && !rpc_url.starts_with("file://")
                {
                    *rpc_url = base_dir.join(rpc_url.as_str()).display().to_string();
                }
            }
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err(AppError::validation(
                    "optional field `history.name` must not be empty when present",
                ));
            }
        }

        self.source.validate()
    }
}

impl HistorySourceConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Jsonl { .. } => "jsonl",
            Self::RpcRange { .. } => "rpc_range",
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        match self {
            Self::Jsonl { path } => {
                if path.as_os_str().is_empty() {
                    return Err(AppError::validation(
                        "required field `history.source.path` must not be empty",
                    ));
                }
            }
            Self::RpcRange {
                rpc_url,
                start_block,
                end_block,
                contract_address,
                method,
            } => {
                require_non_empty("history.source.rpc_url", rpc_url)?;
                require_non_empty("history.source.contract_address", contract_address)?;

                if start_block > end_block {
                    return Err(AppError::validation(format!(
                        "history.source.start_block ({start_block}) must be <= history.source.end_block ({end_block})"
                    )));
                }

                if let Some(method) = method {
                    require_non_empty("history.source.method", method)?;
                }
            }
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn require_non_empty(field: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::validation(format!(
            "required field `{field}` must not be empty"
        )));
    }

    Ok(())
}
