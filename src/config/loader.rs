use std::{fs, path::Path};

use serde::de::DeserializeOwned;

use crate::{
    config::{DexConfig, HistoryConfig},
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfigs {
    pub dex: DexConfig,
    pub history: HistoryConfig,
}

pub fn load_runtime_configs(
    dex_path: impl AsRef<Path>,
    history_path: impl AsRef<Path>,
) -> AppResult<LoadedConfigs> {
    Ok(LoadedConfigs {
        dex: load_dex_config(dex_path)?,
        history: load_history_config(history_path)?,
    })
}

pub fn load_dex_config(path: impl AsRef<Path>) -> AppResult<DexConfig> {
    let path = path.as_ref();
    let mut config: DexConfig = load_toml(path)?;
    if let Some(base_dir) = path.parent() {
        config.resolve_relative_paths(base_dir);
    }
    config.validate()?;
    Ok(config)
}

pub fn load_history_config(path: impl AsRef<Path>) -> AppResult<HistoryConfig> {
    let path = path.as_ref();
    let mut config: HistoryConfig = load_toml(path)?;
    if let Some(base_dir) = path.parent() {
        config.resolve_relative_paths(base_dir);
    }
    config.validate()?;
    Ok(config)
}

fn load_toml<T>(path: impl AsRef<Path>) -> AppResult<T>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| AppError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    toml::from_str(&raw).map_err(|source| AppError::TomlParse {
        path: path.to_path_buf(),
        source,
    })
}
