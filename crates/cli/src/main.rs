use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "structure", about = "Structure: call-graph matrix and impact")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a project and emit the matrix as JSON.
    Analyze {
        path: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Compute the impact of a diff.
    Impact {
        path: PathBuf,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        head: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Analyze { .. } | Command::Impact { .. } => bail!("not implemented yet"),
    }
}
