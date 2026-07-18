use std::{path::PathBuf, process::ExitCode};

use astral::{config::Config, logging, RepositoryRoot};
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "astral", about = "AST-aware repository context engine")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Resolve a repository root and report its index state.
    Status(StatusArgs),
}

#[derive(Debug, clap::Args)]
struct StatusArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> astral::Result<()> {
    let config = Config::from_env()?;
    let cli = Cli::parse();

    let command = match cli.command {
        Some(command) => command,
        None => {
            Cli::command()
                .print_help()
                .map_err(|error| astral::AstralError::Logging {
                    message: error.to_string(),
                })?;
            println!();
            return Ok(());
        }
    };

    logging::init(&config)?;

    match command {
        Commands::Status(args) => status(args),
    }
}

fn status(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    println!("Repository root: {}", root.path().display());
    println!("Index status: not indexed");
    Ok(())
}
