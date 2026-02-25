use std::path::PathBuf;

use crate::{
    backend::{AnvilBackend, EvmBackend, MockBackend},
    cli::{BackendKind, Cli},
    config::loader::{load_runtime_configs, LoadedConfigs},
    dex::{build_adapter, RegisteredDexAdapter},
    engine::EngineRunner,
    error::AppResult,
    history::{build_history_source, HistoryInputSource},
    output::{render_report_summary, write_report, write_report_summary},
    types::RunReport,
};

pub fn run(cli: Cli) -> AppResult<()> {
    let loaded = load_runtime_configs(&cli.dex, &cli.history)?;
    let adapter = build_adapter(loaded.dex.clone())?;
    let mut source = build_history_source(&loaded.history.source)?;

    let (report, raw_path, summary_path) = match cli.backend {
        BackendKind::Mock => {
            let mut backend = MockBackend::new();
            run_pipeline(
                &mut backend,
                &adapter,
                &mut source,
                &loaded,
                &cli.output_dir,
            )?
        }
        BackendKind::Anvil => {
            let mut backend = AnvilBackend::new()?;
            run_pipeline(
                &mut backend,
                &adapter,
                &mut source,
                &loaded,
                &cli.output_dir,
            )?
        }
    };

    println!(
        "completed run: dex=`{}` history_source=`{}` backend=`{:?}`",
        loaded.dex.name,
        loaded.history.source.kind(),
        cli.backend
    );
    println!("raw_output=`{}`", raw_path.display());
    println!("summary_output=`{}`", summary_path.display());
    println!("{}", render_report_summary(&report));

    Ok(())
}

fn run_pipeline<B: EvmBackend>(
    backend: &mut B,
    adapter: &RegisteredDexAdapter,
    source: &mut HistoryInputSource,
    loaded: &LoadedConfigs,
    output_dir: &PathBuf,
) -> AppResult<(RunReport, PathBuf, PathBuf)> {
    let report = EngineRunner::run(backend, adapter, source, &loaded.history.execution)?;
    let raw_path = output_dir.join("run.raw.jsonl");
    let summary_path = output_dir.join("run.summary.txt");

    let raw_path = write_report(&raw_path, &report)?;
    let summary_path = write_report_summary(&summary_path, &report)?;

    Ok((report, raw_path, summary_path))
}
