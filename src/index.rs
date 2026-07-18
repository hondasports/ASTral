use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::{
    analyzer::{LanguageAnalyzer, ReferenceKind, SymbolKind},
    error::{AstralError, Result},
    oxc_analyzer::OxcAnalyzer,
    scanner::SourceScanner,
};

pub(crate) const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStateStatus {
    Fresh,
    Stale,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStateRecord {
    pub relative_path: String,
    pub status: FileStateStatus,
    pub observed_hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStatus {
    pub indexed: bool,
    pub database_path: PathBuf,
    pub schema_version: i64,
    pub file_count: usize,
    pub symbol_count: usize,
    pub diagnostic_count: usize,
    pub stale_count: usize,
    pub missing_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub relative_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolResult {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub relative_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadSymbolResult {
    pub symbol_id: String,
    pub name: String,
    pub kind: String,
    pub relative_path: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IndexStore;

impl IndexStore {
    pub fn default_path(root: &Path) -> PathBuf {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let project_id = hash_bytes(canonical.to_string_lossy().as_bytes());
        let base = ProjectDirs::from("com", "astral", "astral")
            .map(|directories| directories.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".astral"));
        base.join("projects")
            .join(&project_id[..24])
            .join("index.sqlite")
    }

    pub fn rebuild_at(root: &Path, database_path: &Path) -> Result<IndexStatus> {
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent).map_err(|source| AstralError::PathAccess {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let temporary_path = temporary_path(database_path);
        let _ = fs::remove_file(&temporary_path);

        let build_result = (|| {
            let mut connection = Connection::open(&temporary_path).map_err(database_error)?;
            initialize_schema(&connection)?;
            let transaction = connection.transaction().map_err(database_error)?;
            populate(&transaction, root)?;
            transaction.commit().map_err(database_error)?;
            drop(connection);
            replace_database(&temporary_path, database_path)?;
            Self::status_at(database_path)
        })();

        if build_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        build_result
    }

    pub fn rebuild(root: &Path) -> Result<IndexStatus> {
        let database_path = Self::default_path(root);
        Self::rebuild_at(root, &database_path)
    }

    pub fn get_index_status(root: &Path) -> Result<IndexStatus> {
        let database_path = Self::default_path(root);
        Self::status_at(&database_path)
    }

    pub fn search_code(root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        crate::incremental::IncrementalIndexer::new(root, Self::default_path(root)).refresh()?;
        Self::search_code_at(&Self::default_path(root), query)
    }

    pub fn find_symbol(root: &Path, query: &str) -> Result<Vec<SymbolResult>> {
        crate::incremental::IncrementalIndexer::new(root, Self::default_path(root)).refresh()?;
        Self::find_symbol_at(&Self::default_path(root), query)
    }

    pub fn read_symbol(root: &Path, symbol_id: &str) -> Result<ReadSymbolResult> {
        crate::incremental::IncrementalIndexer::new(root, Self::default_path(root)).refresh()?;
        Self::read_symbol_at(&Self::default_path(root), symbol_id)
    }

    pub fn status_at(database_path: &Path) -> Result<IndexStatus> {
        if !database_path.is_file() {
            return Ok(IndexStatus {
                indexed: false,
                database_path: database_path.to_path_buf(),
                schema_version: 0,
                file_count: 0,
                symbol_count: 0,
                diagnostic_count: 0,
                stale_count: 0,
                missing_count: 0,
            });
        }
        let connection = Connection::open(database_path).map_err(database_error)?;
        let schema_version = connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?
            .and_then(|value| value.parse().ok())
            .unwrap_or_default();
        let file_count = count(&connection, "files")?;
        let symbol_count = count(&connection, "symbols")?;
        let diagnostic_count = count(&connection, "diagnostics")?;
        let (stale_count, missing_count) = if table_exists(&connection, "file_states")? {
            (
                count_state(&connection, "stale")?,
                count_state(&connection, "missing")?,
            )
        } else {
            (0, 0)
        };
        Ok(IndexStatus {
            indexed: schema_version == SCHEMA_VERSION,
            database_path: database_path.to_path_buf(),
            schema_version,
            file_count,
            symbol_count,
            diagnostic_count,
            stale_count,
            missing_count,
        })
    }

    pub fn file_state_at(
        database_path: &Path,
        relative_path: &str,
    ) -> Result<Option<FileStateRecord>> {
        let connection = Connection::open(database_path).map_err(database_error)?;
        if !table_exists(&connection, "file_states")? {
            return Ok(None);
        }
        connection
            .query_row(
                "SELECT relative_path, status, observed_hash, error FROM file_states WHERE relative_path = ?1",
                [relative_path],
                |row| {
                    let status: String = row.get(1)?;
                    Ok(FileStateRecord {
                        relative_path: row.get(0)?,
                        status: parse_file_state(&status),
                        observed_hash: row.get(2)?,
                        error: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(database_error)
    }

    pub fn search_code_at(database_path: &Path, query: &str) -> Result<Vec<SearchResult>> {
        let connection = Connection::open(database_path).map_err(database_error)?;
        let mut statement = connection
            .prepare(
                "SELECT c.relative_path, c.start_byte, c.end_byte, c.content
                 FROM chunk_search c JOIN file_states fs ON fs.relative_path = c.relative_path
                 WHERE fs.status = 'fresh' AND chunk_search MATCH ?1 ORDER BY rank LIMIT 100",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map([query], |row| {
                Ok(SearchResult {
                    relative_path: row.get(0)?,
                    start_byte: row.get::<_, i64>(1)? as usize,
                    end_byte: row.get::<_, i64>(2)? as usize,
                    snippet: row.get(3)?,
                })
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)
    }

    pub fn find_symbol_at(database_path: &Path, query: &str) -> Result<Vec<SymbolResult>> {
        let connection = Connection::open(database_path).map_err(database_error)?;
        let like = format!("%{query}%");
        let mut statement = connection
            .prepare(
                "SELECT s.symbol_id, s.name, s.qualified_name, s.kind, f.relative_path,
                        s.start_byte, s.end_byte
                 FROM symbols s JOIN files f ON f.id = s.file_id
                 JOIN file_states fs ON fs.relative_path = f.relative_path AND fs.status = 'fresh'
                 WHERE s.name LIKE ?1 OR s.qualified_name LIKE ?1
                 ORDER BY s.name, f.relative_path LIMIT 100",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map([like], |row| {
                Ok(SymbolResult {
                    symbol_id: row.get(0)?,
                    name: row.get(1)?,
                    qualified_name: row.get(2)?,
                    kind: row.get(3)?,
                    relative_path: row.get(4)?,
                    start_byte: row.get::<_, i64>(5)? as usize,
                    end_byte: row.get::<_, i64>(6)? as usize,
                })
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)
    }

    pub fn read_symbol_at(database_path: &Path, symbol_id: &str) -> Result<ReadSymbolResult> {
        let connection = Connection::open(database_path).map_err(database_error)?;
        connection
            .query_row(
                "SELECT s.symbol_id, s.name, s.kind, f.relative_path, s.start_byte, s.end_byte, f.source
                 FROM symbols s JOIN files f ON f.id = s.file_id
                 JOIN file_states fs ON fs.relative_path = f.relative_path AND fs.status = 'fresh'
                 WHERE s.symbol_id = ?1",
                [symbol_id],
                |row| {
                    let source: String = row.get(6)?;
                    let start = row.get::<_, i64>(4)? as usize;
                    let end = row.get::<_, i64>(5)? as usize;
                    Ok(ReadSymbolResult {
                        symbol_id: row.get(0)?,
                        name: row.get(1)?,
                        kind: row.get(2)?,
                        relative_path: row.get(3)?,
                        source: source.get(start..end).unwrap_or_default().to_owned(),
                    })
                },
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AstralError::Indexing {
                message: format!("symbol not found: {symbol_id}"),
            })
    }
}

fn populate(transaction: &Transaction<'_>, root: &Path) -> Result<()> {
    let scanner = SourceScanner::new(root);
    let files = scanner.scan()?;
    let analyzer = OxcAnalyzer::new(root);
    let mut diagnostic_count = 0_i64;

    transaction
        .execute(
            "INSERT INTO metadata(key, value) VALUES ('schema_version', ?1), ('indexer_version', 'phase-1-oxc')",
            [SCHEMA_VERSION.to_string()],
        )
        .map_err(database_error)?;

    for file in files {
        let analysis = analyzer.analyze(&file.relative_path, &file.source)?;
        diagnostic_count += analysis.diagnostics.len() as i64;
        let hash = hash_bytes(file.source.as_bytes());
        transaction
            .execute(
                "INSERT INTO files(relative_path, language, content_hash, source, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    file.relative_path.to_string_lossy().replace('\\', "/"),
                    file.language,
                    hash,
                    &file.source,
                    file.source.len() as i64,
                ],
            )
            .map_err(database_error)?;
        let file_id = transaction.last_insert_rowid();
        let relative_path = file.relative_path.to_string_lossy().replace('\\', "/");

        for (index, symbol) in analysis.symbols.iter().enumerate() {
            let symbol_id = format!(
                "{relative_path}:{}:{}:{index}",
                symbol.name, symbol.range.start_byte
            );
            transaction
                .execute(
                    "INSERT INTO symbols(symbol_id, file_id, name, qualified_name, kind, start_byte, end_byte, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        symbol_id,
                        file_id,
                        symbol.name,
                        symbol.qualified_name,
                        symbol_kind_name(symbol.kind),
                        symbol.range.start_byte as i64,
                        symbol.range.end_byte as i64,
                        symbol.range.start_line as i64,
                        symbol.range.end_line as i64,
                    ],
                )
                .map_err(database_error)?;
        }
        for reference in &analysis.references {
            transaction
                .execute(
                    "INSERT INTO references_index(file_id, name, target, kind, start_byte, end_byte, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        file_id,
                        reference.name,
                        reference.target,
                        reference_kind_name(reference.kind),
                        reference.range.start_byte as i64,
                        reference.range.end_byte as i64,
                        reference.range.start_line as i64,
                        reference.range.end_line as i64,
                    ],
                )
                .map_err(database_error)?;
        }
        for import in &analysis.imports {
            transaction
                .execute(
                    "INSERT INTO imports(file_id, source, imported_name, local_name, resolved_path) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        file_id,
                        import.source,
                        import.imported_name,
                        import.local_name,
                        import.resolved_path.as_ref().map(|path| path.to_string_lossy().to_string()),
                    ],
                )
                .map_err(database_error)?;
        }
        for export in &analysis.exports {
            transaction
                .execute(
                    "INSERT INTO exports(file_id, exported_name, local_name) VALUES (?1, ?2, ?3)",
                    params![file_id, export.exported_name, export.local_name],
                )
                .map_err(database_error)?;
        }
        for call in &analysis.calls {
            transaction
                .execute(
                    "INSERT INTO calls(file_id, callee, start_byte, end_byte) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        file_id,
                        call.callee,
                        call.range.start_byte as i64,
                        call.range.end_byte as i64,
                    ],
                )
                .map_err(database_error)?;
        }
        for diagnostic in &analysis.diagnostics {
            transaction
                .execute(
                    "INSERT INTO diagnostics(file_id, severity, message) VALUES (?1, ?2, ?3)",
                    params![
                        file_id,
                        format!("{:?}", diagnostic.severity),
                        diagnostic.message
                    ],
                )
                .map_err(database_error)?;
        }
        for (start, end) in chunk_ranges(&file.source) {
            let content = &file.source[start..end];
            transaction
                .execute(
                    "INSERT INTO chunks(file_id, start_byte, end_byte, content) VALUES (?1, ?2, ?3, ?4)",
                    params![file_id, start as i64, end as i64, content],
                )
                .map_err(database_error)?;
            let chunk_id = transaction.last_insert_rowid();
            transaction
                .execute(
                    "INSERT INTO chunk_search(rowid, relative_path, start_byte, end_byte, content) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![chunk_id, relative_path, start as i64, end as i64, content],
                )
                .map_err(database_error)?;
        }
        transaction
            .execute(
                "INSERT INTO file_states(relative_path, status, observed_hash, error, updated_at) VALUES (?1, 'fresh', ?2, NULL, ?3)",
                params![relative_path, hash, now_string()],
            )
            .map_err(database_error)?;
    }
    transaction
        .execute(
            "INSERT INTO metadata(key, value) VALUES ('diagnostic_count', ?1)",
            [diagnostic_count.to_string()],
        )
        .map_err(database_error)?;
    Ok(())
}

fn initialize_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE files(id INTEGER PRIMARY KEY, relative_path TEXT NOT NULL UNIQUE, language TEXT NOT NULL, content_hash TEXT NOT NULL, source TEXT NOT NULL, size_bytes INTEGER NOT NULL);
             CREATE TABLE symbols(id INTEGER PRIMARY KEY, symbol_id TEXT NOT NULL UNIQUE, file_id INTEGER NOT NULL REFERENCES files(id), name TEXT NOT NULL, qualified_name TEXT, kind TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL);
             CREATE TABLE references_index(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), name TEXT NOT NULL, target TEXT, kind TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL);
             CREATE TABLE imports(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), source TEXT NOT NULL, imported_name TEXT, local_name TEXT, resolved_path TEXT);
             CREATE TABLE exports(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), exported_name TEXT NOT NULL, local_name TEXT);
             CREATE TABLE calls(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), callee TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL);
             CREATE TABLE diagnostics(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), severity TEXT NOT NULL, message TEXT NOT NULL);
             CREATE TABLE chunks(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, content TEXT NOT NULL);
             CREATE TABLE file_states(relative_path TEXT PRIMARY KEY, status TEXT NOT NULL, observed_hash TEXT, error TEXT, updated_at TEXT NOT NULL);
             CREATE VIRTUAL TABLE chunk_search USING fts5(relative_path UNINDEXED, start_byte UNINDEXED, end_byte UNINDEXED, content);
            ",
        )
        .map_err(database_error)
}

fn replace_database(temporary_path: &Path, database_path: &Path) -> Result<()> {
    let backup_path = database_path.with_extension("sqlite.previous");
    if backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|source| AstralError::PathAccess {
            path: backup_path.clone(),
            source,
        })?;
    }
    if database_path.exists() {
        fs::rename(database_path, &backup_path).map_err(|source| AstralError::PathAccess {
            path: database_path.to_path_buf(),
            source,
        })?;
    }
    if let Err(source) = fs::rename(temporary_path, database_path) {
        if backup_path.exists() {
            if let Err(restore_error) = fs::rename(&backup_path, database_path) {
                return Err(AstralError::PathAccess {
                    path: database_path.to_path_buf(),
                    source: std::io::Error::other(format!(
                        "failed to activate rebuilt index ({source}); failed to restore previous index ({restore_error})"
                    )),
                });
            }
        }
        return Err(AstralError::PathAccess {
            path: database_path.to_path_buf(),
            source,
        });
    }
    let _ = fs::remove_file(backup_path);
    Ok(())
}

fn count(connection: &Connection, table: &str) -> Result<usize> {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|count| count as usize)
        .map_err(database_error)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(database_error)
}

fn count_state(connection: &Connection, status: &str) -> Result<usize> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM file_states WHERE status = ?1",
            [status],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count as usize)
        .map_err(database_error)
}

fn parse_file_state(status: &str) -> FileStateStatus {
    match status {
        "fresh" => FileStateStatus::Fresh,
        "missing" => FileStateStatus::Missing,
        _ => FileStateStatus::Stale,
    }
}

pub(crate) fn file_state_name(status: FileStateStatus) -> &'static str {
    match status {
        FileStateStatus::Fresh => "fresh",
        FileStateStatus::Stale => "stale",
        FileStateStatus::Missing => "missing",
    }
}

pub(crate) fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
        .to_string()
}

fn temporary_path(database_path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    database_path.with_extension(format!("sqlite.tmp-{}-{stamp}", std::process::id()))
}

pub(crate) fn chunk_ranges(source: &str) -> Vec<(usize, usize)> {
    const MAX_CHUNK_BYTES: usize = 1_200;
    if source.is_empty() {
        return vec![(0, 0)];
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, _) in source.char_indices() {
        if offset.saturating_sub(start) >= MAX_CHUNK_BYTES {
            ranges.push((start, offset));
            start = offset;
        }
    }
    ranges.push((start, source.len()));
    ranges
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn symbol_kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::Type => "type",
        SymbolKind::Variable => "variable",
        SymbolKind::Module => "module",
        SymbolKind::Unknown => "unknown",
    }
}

fn reference_kind_name(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Read => "read",
        ReferenceKind::Write => "write",
        ReferenceKind::Call => "call",
        ReferenceKind::Import => "import",
        ReferenceKind::Unknown => "unknown",
    }
}

pub(crate) fn database_error(error: rusqlite::Error) -> AstralError {
    AstralError::Database {
        message: error.to_string(),
    }
}
