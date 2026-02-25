use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackendKind {
    Mock,
    Anvil,
}

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

    #[arg(
        long,
        value_enum,
        default_value_t = BackendKind::Mock,
        help = "Backend to use for execution"
    )]
    pub backend: BackendKind,

    #[arg(
        long,
        value_name = "DIR",
        default_value = "datasets/cache",
        help = "Directory where raw log and summary will be written"
    )]
    pub output_dir: PathBuf,
}
