use clap::Parser;

fn main() {
    if let Err(error) = dex_sim::app::run(dex_sim::cli::Cli::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
