use std::{path::PathBuf, process::ExitCode};

use astral::{
    config::Config,
    incremental::IncrementalIndexer,
    index::{IndexStore, RelationshipResult},
    logging, RegisteredRepository, RepositoryRegistry,
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
    /// Register a repository name and root path.
    Register(RegisterArgs),
    /// Remove a repository registration and its stored index.
    Unregister(RepositoryArgs),
    /// Resolve a repository root and report its index state.
    Status(RepositoryArgs),
    /// Build or replace the repository index.
    Index(RepositoryArgs),
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
    Watch(RepositoryArgs),
    /// Run the read-only MCP server over stdio.
    Serve,
    /// Evaluate search quality against a JSON dataset.
    Evaluate(EvaluateArgs),
}

#[derive(Debug, clap::Args)]
struct RegisterArgs {
    /// Stable repository name used by CLI and MCP requests.
    repository_name: String,
    /// Repository root path.
    repository_root: PathBuf,
    /// Replace an existing name-to-root mapping.
    #[arg(long)]
    replace: bool,
}

#[derive(Debug, clap::Args)]
struct RepositoryArgs {
    /// Registered repository name.
    repository_name: String,
}

#[derive(Debug, clap::Args)]
struct SearchArgs {
    /// Registered repository name.
    repository_name: String,
    /// Search query or symbol name.
    query: String,
}

#[derive(Debug, clap::Args)]
struct ReadSymbolArgs {
    /// Registered repository name.
    repository_name: String,
    /// Stable symbol identifier returned by find-symbol.
    symbol_id: String,
}

#[derive(Debug, clap::Args)]
struct RelationshipArgs {
    /// Registered repository name.
    repository_name: String,
    /// Symbol identifier, name, or qualified name.
    symbol: String,
}

#[derive(Debug, clap::Args)]
struct EvaluateArgs {
    /// Registered repository name.
    repository_name: String,
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
    let repository = command_repository(&command);
    tracing::info!(
        command = command_name,
        repository = %repository,
        "command started"
    );
    let result = match command {
        Commands::Register(args) => register(args),
        Commands::Unregister(args) => unregister(args),
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
        Commands::Serve => serve().await,
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
        Commands::Register(_) => "register",
        Commands::Unregister(_) => "unregister",
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
        Commands::Serve => "serve",
        Commands::Evaluate(_) => "evaluate",
    }
}

fn command_repository(command: &Commands) -> String {
    match command {
        Commands::Register(args) => args.repository_name.clone(),
        Commands::Unregister(args)
        | Commands::Status(args)
        | Commands::Index(args)
        | Commands::Watch(args) => args.repository_name.clone(),
        Commands::SearchCode(args) | Commands::FindSymbol(args) => args.repository_name.clone(),
        Commands::ReadSymbol(args) => args.repository_name.clone(),
        Commands::FindReferences(args)
        | Commands::FindCallers(args)
        | Commands::FindCallees(args)
        | Commands::FindRelatedTests(args) => args.repository_name.clone(),
        Commands::Evaluate(args) => args.repository_name.clone(),
        Commands::Serve => "<registry>".to_owned(),
    }
}

fn error_kind(error: &astral::AstralError) -> &'static str {
    match error {
        astral::AstralError::PathNotFound { .. } => "path_not_found",
        astral::AstralError::NotDirectory { .. } => "not_directory",
        astral::AstralError::RepositoryRootNotFound { .. } => "repository_root_not_found",
        astral::AstralError::RepositoryNotRegistered { .. } => "repository_not_registered",
        astral::AstralError::InvalidRepositoryName { .. } => "invalid_repository_name",
        astral::AstralError::RepositoryNameConflict { .. } => "repository_name_conflict",
        astral::AstralError::RepositoryRootConflict { .. } => "repository_root_conflict",
        astral::AstralError::PathAccess { .. } => "path_access",
        astral::AstralError::Canonicalize { .. } => "canonicalize",
        astral::AstralError::InvalidConfiguration { .. } => "invalid_configuration",
        astral::AstralError::Logging { .. } => "logging",
        astral::AstralError::Database { .. } => "database",
        astral::AstralError::Indexing { .. } => "indexing",
    }
}

fn register(args: RegisterArgs) -> astral::Result<()> {
    let repository = RepositoryRegistry::new().register(
        &args.repository_name,
        args.repository_root,
        args.replace,
    )?;
    println!(
        "Registered repository '{}' at {}",
        repository.name,
        repository.root.path().display()
    );
    Ok(())
}

fn unregister(args: RepositoryArgs) -> astral::Result<()> {
    let registry = RepositoryRegistry::new();
    let root = registry.registered_root_path(&args.repository_name)?;
    registry.unregister(&args.repository_name)?;
    let removed_index = IndexStore::remove_for_root(&root)?;
    println!(
        "Unregistered repository '{}'{}",
        args.repository_name,
        if removed_index {
            " and removed its index"
        } else {
            ""
        }
    );
    Ok(())
}

fn resolve_repository(name: &str) -> astral::Result<RegisteredRepository> {
    RepositoryRegistry::new().resolve(name)
}

fn status(args: RepositoryArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    println!("Repository: {}", repository.name);
    println!("Repository root: {}", repository.root.path().display());
    let status = IndexStore::get_index_status(repository.root.path())?;
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

fn index(args: RepositoryArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    let database = IndexStore::default_path(repository.root.path());
    let status = IndexStore::rebuild_at(&repository.name, repository.root.path(), &database)?;
    println!(
        "Index updated: {} files, {} symbols",
        status.file_count, status.symbol_count
    );
    Ok(())
}

fn search_code(args: SearchArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    for result in IndexStore::search_code(&repository.name, repository.root.path(), &args.query)? {
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
    let repository = resolve_repository(&args.repository_name)?;
    for result in IndexStore::find_symbol(&repository.name, repository.root.path(), &args.query)? {
        println!(
            "{} {} {} {}",
            result.symbol_id, result.kind, result.relative_path, result.name
        );
    }
    Ok(())
}

fn read_symbol(args: ReadSymbolArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    let result =
        IndexStore::read_symbol(&repository.name, repository.root.path(), &args.symbol_id)?;
    println!(
        "{} {}\n{}",
        result.relative_path, result.name, result.source
    );
    Ok(())
}

fn find_relationships(
    args: RelationshipArgs,
    search: fn(&str, &std::path::Path, &str) -> astral::Result<Vec<RelationshipResult>>,
) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    for result in search(&repository.name, repository.root.path(), &args.symbol)? {
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

fn watch(args: RepositoryArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    let database = IndexStore::default_path(repository.root.path());
    println!("Watching repository: {}", repository.name);
    IncrementalIndexer::new(&repository.name, repository.root.path(), database).watch()
}

async fn serve() -> astral::Result<()> {
    astral::mcp::serve_stdio()
        .await
        .map_err(|message| astral::AstralError::Indexing { message })
}

fn evaluate(args: EvaluateArgs) -> astral::Result<()> {
    let repository = resolve_repository(&args.repository_name)?;
    let dataset = args
        .dataset
        .unwrap_or_else(|| astral::evaluation::default_dataset(repository.root.path()));
    let report = astral::evaluation::evaluate(&repository.name, repository.root.path(), &dataset)?;
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
