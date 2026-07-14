mod adapters;
mod backup;
mod cli;
mod discovery;
mod fault;
mod path_map;
mod plan;
mod rollback;
mod verify;

use clap::Parser;

fn main() {
    if let Err(error) = cli::run(cli::Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
