use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::HistorySourceConfig,
    error::{AppError, AppResult},
    history::{normalize::group_swaps_by_block, source::SwapInputSource},
    types::{BlockSwaps, SwapRequest},
};

#[derive(Debug, Default)]
pub struct JsonlSource {
    blocks: VecDeque<BlockSwaps>,
}

impl JsonlSource {
    pub fn from_config(config: &HistorySourceConfig) -> AppResult<Self> {
        match config {
            HistorySourceConfig::Jsonl { path } => Self::from_path(path),
            other => Err(AppError::validation(format!(
                "expected jsonl history config, got `{}`",
                other.kind()
            ))),
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| AppError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

        let mut swaps = Vec::new();

        for (line_index, line) in raw.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            let swap: SwapRequest = serde_json::from_str(trimmed).map_err(|source| {
                AppError::validation(format!(
                    "failed to parse JSONL at `{}` line {}: {source}",
                    path.display(),
                    line_index + 1
                ))
            })?;

            swap.validate()?;
            swaps.push(swap);
        }

        Self::from_swaps(swaps)
    }

    pub fn from_swaps(swaps: Vec<SwapRequest>) -> AppResult<Self> {
        let blocks = group_swaps_by_block(swaps);
        validate_blocks(&blocks)?;

        Ok(Self {
            blocks: VecDeque::from(blocks),
        })
    }

    pub fn remaining_blocks(&self) -> usize {
        self.blocks.len()
    }
}

impl SwapInputSource for JsonlSource {
    fn next_block(&mut self) -> AppResult<Option<BlockSwaps>> {
        Ok(self.blocks.pop_front())
    }
}

fn validate_blocks(blocks: &[BlockSwaps]) -> AppResult<()> {
    for block in blocks {
        block.validate()?;
    }

    Ok(())
}

#[allow(dead_code)]
fn _path_debug(path: &Path) -> PathBuf {
    path.to_path_buf()
}
