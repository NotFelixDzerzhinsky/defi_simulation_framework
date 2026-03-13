use std::path::PathBuf;

use crate::{
    backend::{AnvilBackend, EvmBackend, MockBackend},
    backend::anvil::ForkConfig,
    cli::{BackendKind, Cli, Command},
    cmd::dump_history,
    config::loader::{load_runtime_configs, LoadedConfigs},
    dex::{build_adapter, RegisteredDexAdapter},
    engine::EngineRunner,
    error::{AppError, AppResult},
    history::{build_history_source, HistoryInputSource},
    output::{render_report_summary, write_report, write_report_summary},
    types::RunReport,
};

pub fn run(cli: Cli) -> AppResult<()> {
    if let Some(command) = &cli.command {
        return run_subcommand(command, &cli);
    }

    let dex_path = cli.dex.as_ref().ok_or_else(|| {
        AppError::validation("--dex is required when running the simulator (no subcommand given)")
    })?;
    let history_path = cli.history.as_ref().ok_or_else(|| {
        AppError::validation(
            "--history is required when running the simulator (no subcommand given)",
        )
    })?;

    let loaded = load_runtime_configs(dex_path, history_path)?;
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
        BackendKind::Fork => {
            let fork_url = cli.fork_url.ok_or_else(|| {
                AppError::validation("--fork-url is required when using --backend=fork")
            })?;

            let fork = ForkConfig {
                fork_url,
                fork_block_number: cli.fork_block,
            };

            let mut backend = AnvilBackend::new_forked(fork)?;
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

fn run_subcommand(command: &Command, _cli: &Cli) -> AppResult<()> {
    match command {
        Command::DumpHistory { history, out } => {
            let out_path = dump_history::resolve_out_path(history, out.as_ref());
            dump_history::run(history, &out_path)
        }
    }
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
