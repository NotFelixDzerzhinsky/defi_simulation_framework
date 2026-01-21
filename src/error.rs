use std::path::PathBuf;

use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),

    #[error("failed to read file `{path}`: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse TOML file `{path}`: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize JSON output: {0}")]
    JsonSerialize(#[from] serde_json::Error),
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}
