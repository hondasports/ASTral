use std::{path::PathBuf, process::ExitCode};

use astral::{config::Config, index::IndexStore, logging, RepositoryRoot};
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
    /// Build or replace the repository index.
    Index(StatusArgs),
    /// Search indexed source code with SQLite FTS5.
    SearchCode(SearchArgs),
    /// Find indexed symbols by name.
    FindSymbol(SearchArgs),
    /// Read an indexed symbol by its stable identifier.
    ReadSymbol(ReadSymbolArgs),
}

#[derive(Debug, clap::Args)]
struct StatusArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
}

#[derive(Debug, clap::Args)]
struct SearchArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
    /// Search query or symbol name.
    query: String,
}

#[derive(Debug, clap::Args)]
struct ReadSymbolArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
    /// Stable symbol identifier returned by find-symbol.
    symbol_id: String,
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
        Commands::Index(args) => index(args),
        Commands::SearchCode(args) => search_code(args),
        Commands::FindSymbol(args) => find_symbol(args),
        Commands::ReadSymbol(args) => read_symbol(args),
    }
}

fn status(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    println!("Repository root: {}", root.path().display());
    let database = IndexStore::default_path(root.path());
    let status = IndexStore::status_at(&database)?;
    if status.indexed {
        println!("Index status: indexed");
        println!("Indexed files: {}", status.file_count);
        println!("Indexed symbols: {}", status.symbol_count);
        println!("Diagnostics: {}", status.diagnostic_count);
    } else {
        println!("Index status: not indexed");
    }
    Ok(())
}

fn index(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let database = IndexStore::default_path(root.path());
    let status = IndexStore::rebuild_at(root.path(), &database)?;
    println!(
        "Index updated: {} files, {} symbols",
        status.file_count, status.symbol_count
    );
    Ok(())
}

fn search_code(args: SearchArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let database = IndexStore::default_path(root.path());
    for result in IndexStore::search_code_at(&database, &args.query)? {
        println!(
            "{}:{}-{}\n{}",
            result.relative_path, result.start_byte, result.end_byte, result.snippet
        );
    }
    Ok(())
}

fn find_symbol(args: SearchArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let database = IndexStore::default_path(root.path());
    for result in IndexStore::find_symbol_at(&database, &args.query)? {
        println!(
            "{} {} {} {}",
            result.symbol_id, result.kind, result.relative_path, result.name
        );
    }
    Ok(())
}

fn read_symbol(args: ReadSymbolArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let database = IndexStore::default_path(root.path());
    let result = IndexStore::read_symbol_at(&database, &args.symbol_id)?;
    println!(
        "{} {}\n{}",
        result.relative_path, result.name, result.source
    );
    Ok(())
}
