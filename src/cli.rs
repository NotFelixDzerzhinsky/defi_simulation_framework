use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackendKind {
    Mock,
    Anvil,
    Fork,
}

#[derive(Debug, Clone, Parser)]
#[command(
    name = "dex-sim",
    version,
    about = "Deterministic DEX swap simulator",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(long, value_name = "PATH", help = "Path to dex.toml")]
    pub dex: Option<PathBuf>,

    #[arg(long, value_name = "PATH", help = "Path to history.toml")]
    pub history: Option<PathBuf>,

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

    #[arg(
        long,
        value_name = "URL",
        help = "RPC URL to fork from (required for --backend=fork)"
    )]
    pub fork_url: Option<String>,

    #[arg(
        long,
        value_name = "BLOCK",
        help = "Block number to fork at (optional, used with --backend=fork)"
    )]
    pub fork_block: Option<u64>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    DumpHistory {
        #[arg(long, value_name = "PATH")]
        history: PathBuf,

        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
    },
}
