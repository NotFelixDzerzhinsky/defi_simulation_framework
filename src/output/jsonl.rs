use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{AppError, AppResult},
    types::{RunReport, SwapExecutionResult},
};

pub fn flatten_report(report: &RunReport) -> Vec<SwapExecutionResult> {
    report
        .blocks
        .iter()
        .flat_map(|block| block.swaps.iter().cloned())
        .collect()
}

pub fn serialize_results(results: &[SwapExecutionResult]) -> AppResult<String> {
    let mut lines = Vec::with_capacity(results.len());

    for result in results {
        lines.push(serde_json::to_string(result)?);
    }

    Ok(lines.join("\n"))
}

pub fn serialize_report(report: &RunReport) -> AppResult<String> {
    serialize_results(&flatten_report(report))
}

pub fn write_results(
    path: impl AsRef<Path>,
    results: &[SwapExecutionResult],
) -> AppResult<PathBuf> {
    write_jsonl(path, &serialize_results(results)?)
}

pub fn write_report(path: impl AsRef<Path>, report: &RunReport) -> AppResult<PathBuf> {
    write_jsonl(path, &serialize_report(report)?)
}

fn write_jsonl(path: impl AsRef<Path>, content: &str) -> AppResult<PathBuf> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, content).map_err(|source| AppError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(path.to_path_buf())
}
