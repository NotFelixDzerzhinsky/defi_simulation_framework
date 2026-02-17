use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{AppError, AppResult},
    types::{RunReport, RunSummary},
};

pub fn render_summary(summary: &RunSummary) -> String {
    format!(
        "processed_blocks={} total_swaps={} successes={} reverts={} skipped={} total_gas_used={}",
        summary.processed_blocks,
        summary.total_swaps,
        summary.successes,
        summary.reverts,
        summary.skipped,
        summary.total_gas_used
    )
}

pub fn render_report_summary(report: &RunReport) -> String {
    render_summary(&report.summary)
}

pub fn write_summary(path: impl AsRef<Path>, summary: &RunSummary) -> AppResult<PathBuf> {
    write_text(path, &render_summary(summary))
}

pub fn write_report_summary(path: impl AsRef<Path>, report: &RunReport) -> AppResult<PathBuf> {
    write_text(path, &render_report_summary(report))
}

fn write_text(path: impl AsRef<Path>, content: &str) -> AppResult<PathBuf> {
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
