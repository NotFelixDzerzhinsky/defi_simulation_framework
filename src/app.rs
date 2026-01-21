use crate::{cli::Cli, config::loader::load_runtime_configs, error::AppResult};

pub fn run(cli: Cli) -> AppResult<()> {
    let loaded = load_runtime_configs(&cli.dex, &cli.history)?;

    println!(
        "validated configs: dex=`{}` history_source=`{}`",
        loaded.dex.name,
        loaded.history.source.kind()
    );

    Ok(())
}
