use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "dex-sim",
    version,
    about = "Deterministic DEX swap simulator",
    long_about = None
)]
pub struct Cli {
    #[arg(long, value_name = "PATH", help = "Path to dex.toml")]
    pub dex: PathBuf,

    #[arg(long, value_name = "PATH", help = "Path to history.toml")]
    pub history: PathBuf,
}
