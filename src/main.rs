use std::{env::current_dir, error::Error, path::PathBuf};

use clap::Parser;
use netmap::Network;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Config file to load
    file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();
    let cli = Cli::parse();

    let mut path = current_dir().unwrap();
    path.push(cli.file.unwrap_or_else(|| PathBuf::from("network.json")));

    let mut network = Network::try_from(path.as_ref())?;
    network.poll()?;
    println!("{}", network.map());

    Ok(())
}
