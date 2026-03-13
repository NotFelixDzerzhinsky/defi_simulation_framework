use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    config::loader::load_history_config,
    error::{AppError, AppResult},
    history::{build_history_source, SwapInputSource},
};

pub fn run(history_path: &Path, out_path: &Path) -> AppResult<()> {
    let config = load_history_config(history_path)?;
    let mut source = build_history_source(&config.source)?;

    let blocks = source.collect_all()?;

    let total_swaps: usize = blocks.iter().map(|b| b.swaps.len()).sum();

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| AppError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }

    let mut file = fs::File::create(out_path).map_err(|source| AppError::WriteFile {
        path: out_path.to_path_buf(),
        source,
    })?;

    for block in &blocks {
        for swap in &block.swaps {
            let line = serde_json::to_string(swap)?;
            writeln!(file, "{line}").map_err(|source| AppError::WriteFile {
                path: out_path.to_path_buf(),
                source,
            })?;
        }
    }

    println!(
        "dump-history: source=`{}` blocks={} swaps={} output=`{}`",
        config.source.kind(),
        blocks.len(),
        total_swaps,
        out_path.display(),
    );

    Ok(())
}

pub fn resolve_out_path(history_path: &Path, explicit_out: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = explicit_out {
        return p.clone();
    }

    // Default: same directory as history.toml
    let base = history_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    base.join("history.dump.jsonl")
}
