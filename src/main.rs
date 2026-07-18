use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use astral::{
    config::Config,
    incremental::IncrementalIndexer,
    index::{IndexStore, RelationshipResult},
    logging, RepositoryRoot,
};
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
    /// Find symbols that reference the target symbol.
    FindReferences(RelationshipArgs),
    /// Find callers of the target symbol.
    FindCallers(RelationshipArgs),
    /// Find symbols called by the source symbol.
    FindCallees(RelationshipArgs),
    /// Find tests related to the target symbol.
    FindRelatedTests(RelationshipArgs),
    /// Watch the Working Tree and incrementally update the index.
    Watch(StatusArgs),
    /// Run the read-only MCP server over stdio.
    Serve(StatusArgs),
    /// Evaluate search quality against a JSON dataset.
    Evaluate(EvaluateArgs),
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

#[derive(Debug, clap::Args)]
struct RelationshipArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
    /// Symbol identifier, name, or qualified name.
    symbol: String,
}

#[derive(Debug, clap::Args)]
struct EvaluateArgs {
    /// A repository directory or a directory below its root.
    repository_root: PathBuf,
    /// Evaluation dataset JSON path.
    dataset: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> astral::Result<()> {
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

    let command_name = command_name(&command);
    let repository = command_repository(&command).display().to_string();
    tracing::info!(
        command = command_name,
        repository = %repository,
        "command started"
    );
    let result = match command {
        Commands::Status(args) => status(args),
        Commands::Index(args) => index(args),
        Commands::SearchCode(args) => search_code(args),
        Commands::FindSymbol(args) => find_symbol(args),
        Commands::ReadSymbol(args) => read_symbol(args),
        Commands::FindReferences(args) => find_relationships(args, IndexStore::find_references),
        Commands::FindCallers(args) => find_relationships(args, IndexStore::find_callers),
        Commands::FindCallees(args) => find_relationships(args, IndexStore::find_callees),
        Commands::FindRelatedTests(args) => {
            find_relationships(args, IndexStore::find_related_tests)
        }
        Commands::Watch(args) => watch(args),
        Commands::Serve(args) => serve(args).await,
        Commands::Evaluate(args) => evaluate(args),
    };
    match &result {
        Ok(()) => tracing::info!(
            command = command_name,
            repository = %repository,
            "command completed"
        ),
        Err(error) => tracing::error!(
            command = command_name,
            repository = %repository,
            error_kind = error_kind(error),
            "command failed"
        ),
    }
    result
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Status(_) => "status",
        Commands::Index(_) => "index",
        Commands::SearchCode(_) => "search-code",
        Commands::FindSymbol(_) => "find-symbol",
        Commands::ReadSymbol(_) => "read-symbol",
        Commands::FindReferences(_) => "find-references",
        Commands::FindCallers(_) => "find-callers",
        Commands::FindCallees(_) => "find-callees",
        Commands::FindRelatedTests(_) => "find-related-tests",
        Commands::Watch(_) => "watch",
        Commands::Serve(_) => "serve",
        Commands::Evaluate(_) => "evaluate",
    }
}

fn command_repository(command: &Commands) -> &Path {
    match command {
        Commands::Status(args)
        | Commands::Index(args)
        | Commands::Watch(args)
        | Commands::Serve(args) => &args.repository_root,
        Commands::SearchCode(args) | Commands::FindSymbol(args) => &args.repository_root,
        Commands::ReadSymbol(args) => &args.repository_root,
        Commands::FindReferences(args)
        | Commands::FindCallers(args)
        | Commands::FindCallees(args)
        | Commands::FindRelatedTests(args) => &args.repository_root,
        Commands::Evaluate(args) => &args.repository_root,
    }
}

fn error_kind(error: &astral::AstralError) -> &'static str {
    match error {
        astral::AstralError::PathNotFound { .. } => "path_not_found",
        astral::AstralError::NotDirectory { .. } => "not_directory",
        astral::AstralError::RepositoryRootNotFound { .. } => "repository_root_not_found",
        astral::AstralError::PathAccess { .. } => "path_access",
        astral::AstralError::Canonicalize { .. } => "canonicalize",
        astral::AstralError::InvalidConfiguration { .. } => "invalid_configuration",
        astral::AstralError::Logging { .. } => "logging",
        astral::AstralError::Database { .. } => "database",
        astral::AstralError::Indexing { .. } => "indexing",
    }
}

fn status(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    println!("Repository root: {}", root.path().display());
    let status = IndexStore::get_index_status(root.path())?;
    if status.indexed {
        println!("Index status: indexed");
        println!("Indexed files: {}", status.file_count);
        println!("Indexed symbols: {}", status.symbol_count);
        println!("Diagnostics: {}", status.diagnostic_count);
        println!("Stale files: {}", status.stale_count);
        println!("Missing files: {}", status.missing_count);
        if let Some(head) = status.snapshot_head {
            println!("Indexed HEAD: {head}");
            println!(
                "Working Tree: {} ({} dirty files)",
                if status.working_tree_dirty {
                    "dirty"
                } else {
                    "clean"
                },
                status.dirty_file_count
            );
        }
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
    for result in IndexStore::search_code(root.path(), &args.query)? {
        println!(
            "{}:{}-{} score={:.3} matched_by={} reason={}\n{}",
            result.relative_path,
            result.start_byte,
            result.end_byte,
            result.score,
            result.matched_by.join(","),
            result.reason,
            result.snippet
        );
    }
    Ok(())
}

fn find_symbol(args: SearchArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    for result in IndexStore::find_symbol(root.path(), &args.query)? {
        println!(
            "{} {} {} {}",
            result.symbol_id, result.kind, result.relative_path, result.name
        );
    }
    Ok(())
}

fn read_symbol(args: ReadSymbolArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let result = IndexStore::read_symbol(root.path(), &args.symbol_id)?;
    println!(
        "{} {}\n{}",
        result.relative_path, result.name, result.source
    );
    Ok(())
}

fn find_relationships(
    args: RelationshipArgs,
    search: fn(&std::path::Path, &str) -> astral::Result<Vec<RelationshipResult>>,
) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    for result in search(root.path(), &args.symbol)? {
        println!(
            "{} {:.1} {} {} -> {} {}",
            result.edge_type,
            result.confidence,
            result.resolution_method,
            result.source_file,
            result.target_file.as_deref().unwrap_or("<external>"),
            result
                .target_name
                .as_deref()
                .or(result.target_external_name.as_deref())
                .unwrap_or("<unknown>")
        );
    }
    Ok(())
}

fn watch(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let database = IndexStore::default_path(root.path());
    println!("Watching repository: {}", root.path().display());
    IncrementalIndexer::new(root.path(), database).watch()
}

async fn serve(args: StatusArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    std::env::set_current_dir(root.path()).map_err(|source| astral::AstralError::PathAccess {
        path: root.path().to_path_buf(),
        source,
    })?;
    astral::mcp::serve_stdio()
        .await
        .map_err(|message| astral::AstralError::Indexing { message })
}

fn evaluate(args: EvaluateArgs) -> astral::Result<()> {
    let root = RepositoryRoot::resolve(args.repository_root)?;
    let dataset = args
        .dataset
        .unwrap_or_else(|| astral::evaluation::default_dataset(root.path()));
    let report = astral::evaluation::evaluate(root.path(), &dataset)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| {
            astral::AstralError::InvalidConfiguration {
                message: error.to_string(),
            }
        })?
    );
    Ok(())
}
