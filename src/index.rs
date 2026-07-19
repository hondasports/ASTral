use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::{
    analyzer::{LanguageAnalyzer, ReferenceKind, SymbolKind},
    error::{AstralError, Result},
    oxc_analyzer::OxcAnalyzer,
    repository::default_data_dir,
    scanner::SourceScanner,
};

pub(crate) const SCHEMA_VERSION: i64 = 5;

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
    pub snapshot_head: Option<String>,
    pub snapshot_branch: Option<String>,
    pub working_tree_dirty: bool,
    pub dirty_file_count: usize,
    pub snapshot_stale: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub relative_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub snippet: String,
    pub score: f64,
    pub matched_by: Vec<String>,
    pub reason: String,
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

#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipResult {
    pub edge_type: String,
    pub confidence: f64,
    pub resolution_method: String,
    pub source_file: String,
    pub source_symbol_id: Option<String>,
    pub source_name: Option<String>,
    pub target_file: Option<String>,
    pub target_symbol_id: Option<String>,
    pub target_name: Option<String>,
    pub target_external_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IndexStore;

impl IndexStore {
    pub fn default_path(root: &Path) -> PathBuf {
        Self::path_in_data_dir(root, &default_data_dir())
    }

    pub fn path_in_data_dir(root: &Path, data_dir: &Path) -> PathBuf {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let project_id = hash_bytes(canonical.to_string_lossy().as_bytes());
        data_dir
            .join("projects")
            .join(&project_id[..24])
            .join("index.sqlite")
    }

    pub fn remove_for_root(root: &Path) -> Result<bool> {
        let database_path = Self::default_path(root);
        let previous_path = database_path.with_extension("sqlite.previous");
        let mut removed = false;
        for path in [database_path, previous_path] {
            if path.is_file() {
                fs::remove_file(&path).map_err(|source| AstralError::PathAccess {
                    path: path.clone(),
                    source,
                })?;
                removed = true;
            }
        }
        Ok(removed)
    }

    pub fn rebuild_at(
        repository_name: &str,
        root: &Path,
        database_path: &Path,
    ) -> Result<IndexStatus> {
        crate::repository::validate_repository_name(repository_name)?;
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent).map_err(|source| AstralError::PathAccess {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let temporary_path = temporary_path(database_path);
        let _ = fs::remove_file(&temporary_path);
        let started_at = Instant::now();
        tracing::info!(repository = %root.display(), "indexing started");

        let build_result = (|| {
            let mut connection = Connection::open(&temporary_path).map_err(database_error)?;
            initialize_schema(&connection)?;
            let transaction = connection.transaction().map_err(database_error)?;
            populate(&transaction, repository_name, root)?;
            transaction.commit().map_err(database_error)?;
            drop(connection);
            replace_database(&temporary_path, database_path)?;
            Self::status_at(database_path)
        })();

        if build_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        match build_result {
            Ok(status) => {
                tracing::info!(
                    files = status.file_count,
                    symbols = status.symbol_count,
                    diagnostics = status.diagnostic_count,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "indexing completed"
                );
                Ok(status)
            }
            Err(error) => {
                tracing::error!(
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "indexing failed"
                );
                Err(error)
            }
        }
    }

    pub fn rebuild(repository_name: &str, root: &Path) -> Result<IndexStatus> {
        let database_path = Self::default_path(root);
        Self::rebuild_at(repository_name, root, &database_path)
    }

    pub fn get_index_status(root: &Path) -> Result<IndexStatus> {
        let database_path = Self::default_path(root);
        Self::status_at(&database_path)
    }

    pub fn search_code(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<SearchResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::search_code_at(&Self::default_path(root), query)
    }

    pub fn find_symbol(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<SymbolResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::find_symbol_at(&Self::default_path(root), query)
    }

    pub fn read_symbol(
        repository_name: &str,
        root: &Path,
        symbol_id: &str,
    ) -> Result<ReadSymbolResult> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::read_symbol_at(&Self::default_path(root), symbol_id)
    }

    pub fn find_references(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<RelationshipResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::find_relationships_at(&Self::default_path(root), query, "reference", false)
    }

    pub fn find_callers(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<RelationshipResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::find_relationships_at(&Self::default_path(root), query, "call", false)
    }

    pub fn find_callees(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<RelationshipResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::find_relationships_at(&Self::default_path(root), query, "call", true)
    }

    pub fn find_related_tests(
        repository_name: &str,
        root: &Path,
        query: &str,
    ) -> Result<Vec<RelationshipResult>> {
        crate::incremental::IncrementalIndexer::new(
            repository_name,
            root,
            Self::default_path(root),
        )
        .refresh()?;
        Self::find_relationships_at(&Self::default_path(root), query, "test", false)
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
                snapshot_head: None,
                snapshot_branch: None,
                working_tree_dirty: false,
                dirty_file_count: 0,
                snapshot_stale: false,
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
        let git_available =
            metadata_value(&connection, "git_available")?.is_some_and(|value| value == "true");
        Ok(IndexStatus {
            indexed: schema_version == SCHEMA_VERSION,
            database_path: database_path.to_path_buf(),
            schema_version,
            file_count,
            symbol_count,
            diagnostic_count,
            stale_count,
            missing_count,
            snapshot_head: git_available
                .then(|| metadata_value(&connection, "git_head").ok().flatten())
                .flatten(),
            snapshot_branch: git_available
                .then(|| metadata_value(&connection, "git_branch").ok().flatten())
                .flatten(),
            working_tree_dirty: metadata_value(&connection, "git_dirty")?
                .is_some_and(|value| value == "true"),
            dirty_file_count: metadata_value(&connection, "git_dirty_file_count")?
                .and_then(|value| value.parse().ok())
                .unwrap_or_default(),
            snapshot_stale: false,
        })
    }

    pub(crate) fn repository_name_at(database_path: &Path) -> Result<Option<String>> {
        if !database_path.is_file() {
            return Ok(None);
        }
        let connection = Connection::open(database_path).map_err(database_error)?;
        metadata_value(&connection, "repository_name")
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
        let fts_query = crate::ranking::fts_query(query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = connection
            .prepare(
                "SELECT c.relative_path, c.start_byte, c.end_byte, c.content, rank,
                        (SELECT COUNT(*) FROM symbol_edges e
                         WHERE e.source_file_id = f.id OR e.target_file_id = f.id) AS graph_degree
                 FROM chunk_search c
                 JOIN files f ON f.relative_path = c.relative_path
                 JOIN file_states fs ON fs.relative_path = c.relative_path
                 WHERE fs.status = 'fresh' AND chunk_search MATCH ?1 ORDER BY rank LIMIT 100",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map([fts_query], |row| {
                let relative_path: String = row.get(0)?;
                let snippet: String = row.get(3)?;
                let explanation = crate::ranking::explain(
                    query,
                    &relative_path,
                    &snippet,
                    row.get(4)?,
                    row.get::<_, i64>(5)? as usize,
                );
                Ok(SearchResult {
                    relative_path,
                    start_byte: row.get::<_, i64>(1)? as usize,
                    end_byte: row.get::<_, i64>(2)? as usize,
                    snippet,
                    score: explanation.score,
                    matched_by: explanation.matched_by,
                    reason: explanation.reason,
                })
            })
            .map_err(database_error)?;
        let mut results = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?;
        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
                .then_with(|| left.start_byte.cmp(&right.start_byte))
        });
        Ok(results)
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

    pub fn find_relationships_at(
        database_path: &Path,
        query: &str,
        edge_type: &str,
        source_matches: bool,
    ) -> Result<Vec<RelationshipResult>> {
        let connection = Connection::open(database_path).map_err(database_error)?;
        let condition = if source_matches {
            "(e.source_symbol_id = ?2 OR ss.name = ?2 OR ss.qualified_name = ?2)"
        } else {
            "(e.target_symbol_id = ?2 OR ts.name = ?2 OR ts.qualified_name = ?2 OR e.target_external_name = ?2)"
        };
        let sql = format!(
            "SELECT e.edge_type, e.confidence, e.resolution_method,
                    sf.relative_path, e.source_symbol_id, ss.name,
                    tf.relative_path, e.target_symbol_id, ts.name, e.target_external_name
             FROM symbol_edges e
             JOIN files sf ON sf.id = e.source_file_id
             LEFT JOIN symbols ss ON ss.symbol_id = e.source_symbol_id
             LEFT JOIN files tf ON tf.id = e.target_file_id
             LEFT JOIN symbols ts ON ts.symbol_id = e.target_symbol_id
             JOIN file_states sfs ON sfs.relative_path = sf.relative_path AND sfs.status = 'fresh'
             LEFT JOIN file_states tfs ON tfs.relative_path = tf.relative_path
             WHERE e.edge_type = ?1 AND {condition}
               AND (tfs.status IS NULL OR tfs.status = 'fresh')
             ORDER BY sf.relative_path, ss.name, tf.relative_path, ts.name"
        );
        let mut statement = connection.prepare(&sql).map_err(database_error)?;
        let rows = statement
            .query_map(rusqlite::params![edge_type, query], |row| {
                Ok(RelationshipResult {
                    edge_type: row.get(0)?,
                    confidence: row.get(1)?,
                    resolution_method: row.get(2)?,
                    source_file: row.get(3)?,
                    source_symbol_id: row.get(4)?,
                    source_name: row.get(5)?,
                    target_file: row.get(6)?,
                    target_symbol_id: row.get(7)?,
                    target_name: row.get(8)?,
                    target_external_name: row.get(9)?,
                })
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)
    }
}

fn populate(transaction: &Transaction<'_>, repository_name: &str, root: &Path) -> Result<()> {
    let scanner = SourceScanner::new(root);
    let files = scanner.scan()?;
    let total_files = files.len();
    tracing::info!(files = total_files, "indexing files discovered");
    let analyzer = OxcAnalyzer::new(root);
    let mut diagnostic_count = 0_i64;

    transaction
        .execute(
        "INSERT INTO metadata(key, value) VALUES ('schema_version', ?1), ('indexer_version', 'phase-5-git'), ('repository_name', ?2)",
            params![SCHEMA_VERSION.to_string(), repository_name],
        )
        .map_err(database_error)?;
    let git = crate::git::inspect(root)?;
    transaction
        .execute(
            "INSERT INTO metadata(key, value) VALUES ('git_available', ?1), ('git_head', ?2), ('git_branch', ?3), ('git_dirty', ?4), ('git_dirty_file_count', ?5), ('git_worktree_hash', ?6)",
            params![
                git.available.to_string(),
                git.head.unwrap_or_default(),
                git.branch.unwrap_or_default(),
                git.dirty.to_string(),
                git.dirty_file_count.to_string(),
                git.worktree_hash.unwrap_or_default()
            ],
        )
        .map_err(database_error)?;

    for (index, file) in files.into_iter().enumerate() {
        let current = index + 1;
        let relative_path = file.relative_path.to_string_lossy().replace('\\', "/");
        tracing::info!(
            current,
            total = total_files,
            path = %relative_path,
            "indexing file"
        );
        let analysis = analyzer.analyze(&file.relative_path, &file.source)?;
        diagnostic_count += analysis.diagnostics.len() as i64;
        let hash = hash_bytes(file.source.as_bytes());
        transaction
            .execute(
                "INSERT INTO files(relative_path, language, content_hash, public_api_hash, source, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    file.relative_path.to_string_lossy().replace('\\', "/"),
                    file.language,
                    hash,
                    public_api_hash(&analysis),
                    &file.source,
                    file.source.len() as i64,
                ],
            )
            .map_err(database_error)?;
        let file_id = transaction.last_insert_rowid();
        for (index, symbol) in analysis.symbols.iter().enumerate() {
            let symbol_id = symbol_id(
                repository_name,
                &relative_path,
                symbol.name.as_str(),
                symbol.range.start_byte,
                index,
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
        rebuild_edges(transaction)?;
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

#[derive(Debug, Clone)]
struct EdgeFile {
    id: i64,
    path: String,
}

#[derive(Debug, Clone)]
struct EdgeSymbol {
    id: String,
    file_id: i64,
    name: String,
    start: i64,
    end: i64,
}

#[derive(Debug, Clone)]
struct ImportLink {
    target_file_id: Option<i64>,
    imported_name: Option<String>,
}

type ImportRecord = (i64, Option<String>, Option<String>, Option<String>);

pub(crate) fn public_api_hash(analysis: &crate::analyzer::AnalysisResult) -> String {
    let mut exports: Vec<_> = analysis
        .exports
        .iter()
        .map(|export| {
            format!(
                "{}:{}",
                export.exported_name,
                export.local_name.as_deref().unwrap_or_default()
            )
        })
        .collect();
    exports.sort();
    hash_bytes(exports.join("\n").as_bytes())
}

pub(crate) fn rebuild_edges(transaction: &Transaction<'_>) -> Result<()> {
    transaction
        .execute("DELETE FROM symbol_edges", [])
        .map_err(database_error)?;

    let files: Vec<EdgeFile> = {
        let mut statement = transaction
            .prepare("SELECT id, relative_path FROM files")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(EdgeFile {
                    id: row.get(0)?,
                    path: row.get(1)?,
                })
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    let files_by_path: HashMap<String, EdgeFile> = files
        .iter()
        .cloned()
        .map(|file| (file.path.clone(), file))
        .collect();
    let symbols: Vec<EdgeSymbol> = {
        let mut statement = transaction
            .prepare("SELECT symbol_id, file_id, name, start_byte, end_byte FROM symbols")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(EdgeSymbol {
                    id: row.get(0)?,
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    start: row.get(3)?,
                    end: row.get(4)?,
                })
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    let symbols_by_file_name: HashMap<(i64, String), Vec<EdgeSymbol>> = symbols
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut map, symbol| {
            map.entry((symbol.file_id, symbol.name.clone()))
                .or_default()
                .push(symbol);
            map
        });
    let mut imports_by_file: HashMap<(i64, String), ImportLink> = HashMap::new();
    {
        let mut statement = transaction
            .prepare("SELECT file_id, local_name, imported_name, resolved_path FROM imports")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(database_error)?;
        for row in rows {
            let (file_id, local_name, imported_name, resolved_path) =
                row.map_err(database_error)?;
            if let Some(local_name) = local_name {
                imports_by_file.insert(
                    (file_id, local_name),
                    ImportLink {
                        target_file_id: resolved_path
                            .map(|path| path.replace('\\', "/"))
                            .and_then(|path| files_by_path.get(&path).map(|file| file.id)),
                        imported_name,
                    },
                );
            }
        }
    }

    let references: Vec<(i64, String, Option<String>, i64, i64)> = {
        let mut statement = transaction
            .prepare("SELECT file_id, name, target, start_byte, end_byte FROM references_index")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    for (file_id, name, target, start, end) in references {
        let source = enclosing_symbol(&symbols, file_id, start, end);
        let target_name = target.as_deref().unwrap_or(&name);
        let resolved = resolve_target(
            &symbols_by_file_name,
            &imports_by_file,
            file_id,
            target_name,
        );
        insert_edge(
            transaction,
            file_id,
            source.map(|symbol| symbol.id.as_str()),
            resolved.0,
            resolved.1.as_deref(),
            resolved.2,
            "reference",
            resolved.3,
            resolved.4,
        )?;
    }

    let calls: Vec<(i64, String, i64, i64)> = {
        let mut statement = transaction
            .prepare("SELECT file_id, callee, start_byte, end_byte FROM calls")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    for (file_id, callee, start, end) in calls {
        let source = enclosing_symbol(&symbols, file_id, start, end);
        let resolved = resolve_target(&symbols_by_file_name, &imports_by_file, file_id, &callee);
        insert_edge(
            transaction,
            file_id,
            source.map(|symbol| symbol.id.as_str()),
            resolved.0,
            resolved.1.as_deref(),
            resolved.2,
            "call",
            resolved.3,
            resolved.4,
        )?;
    }

    let imports: Vec<ImportRecord> = {
        let mut statement = transaction
            .prepare("SELECT file_id, source, imported_name, resolved_path FROM imports")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    for (file_id, source, imported_name, resolved_path) in imports {
        let target_file_id = resolved_path
            .as_deref()
            .map(|path| path.replace('\\', "/"))
            .and_then(|path| files_by_path.get(&path).map(|file| file.id));
        let target_name = imported_name.as_deref().filter(|name| *name != "*");
        let target_symbol = target_file_id.and_then(|target_file_id| {
            target_name.and_then(|name| {
                symbols_by_file_name
                    .get(&(target_file_id, name.to_owned()))
                    .and_then(|symbols| symbols.first())
            })
        });
        let confidence = if target_symbol.is_some() {
            1.0
        } else if target_file_id.is_some() {
            0.7
        } else {
            0.4
        };
        let method = if target_symbol.is_some() {
            "semantic"
        } else if target_file_id.is_some() {
            "ast"
        } else {
            "heuristic"
        };
        insert_edge(
            transaction,
            file_id,
            None,
            target_file_id,
            target_symbol.map(|symbol| symbol.id.as_str()),
            imported_name.or(source),
            "import",
            confidence,
            method,
        )?;
    }

    let exports: Vec<(i64, String, Option<String>)> = {
        let mut statement = transaction
            .prepare("SELECT file_id, exported_name, local_name FROM exports")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(database_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };
    for (file_id, exported_name, local_name) in exports {
        let target_symbol = local_name.as_deref().and_then(|name| {
            symbols_by_file_name
                .get(&(file_id, name.to_owned()))
                .and_then(|symbols| symbols.first())
        });
        insert_edge(
            transaction,
            file_id,
            None,
            Some(file_id),
            target_symbol.map(|symbol| symbol.id.as_str()),
            Some(exported_name),
            "export",
            if target_symbol.is_some() { 1.0 } else { 0.7 },
            if target_symbol.is_some() {
                "semantic"
            } else {
                "ast"
            },
        )?;
    }

    for test_file in files.iter().filter(|file| is_test_path(&file.path)) {
        let base = test_base_name(&test_file.path);
        for implementation in files
            .iter()
            .filter(|file| !is_test_path(&file.path) && path_base_name(&file.path) == base)
        {
            let implementation_symbols = symbols
                .iter()
                .filter(|symbol| symbol.file_id == implementation.id);
            for symbol in implementation_symbols {
                insert_edge(
                    transaction,
                    test_file.id,
                    None,
                    Some(implementation.id),
                    Some(symbol.id.as_str()),
                    None,
                    "test",
                    0.4,
                    "heuristic",
                )?;
            }
        }
    }
    transaction
        .execute(
            "DELETE FROM symbol_edges WHERE id NOT IN (
                SELECT MIN(id) FROM symbol_edges
                GROUP BY source_file_id, source_symbol_id, target_file_id, target_symbol_id, target_external_name, edge_type
            )",
            [],
        )
        .map_err(database_error)?;
    Ok(())
}

fn enclosing_symbol(
    symbols: &[EdgeSymbol],
    file_id: i64,
    start: i64,
    end: i64,
) -> Option<&EdgeSymbol> {
    symbols
        .iter()
        .filter(|symbol| symbol.file_id == file_id && symbol.start <= start && symbol.end >= end)
        .min_by_key(|symbol| symbol.end - symbol.start)
}

fn resolve_target(
    symbols_by_file_name: &HashMap<(i64, String), Vec<EdgeSymbol>>,
    imports_by_file: &HashMap<(i64, String), ImportLink>,
    source_file_id: i64,
    name: &str,
) -> (
    Option<i64>,
    Option<String>,
    Option<String>,
    f64,
    &'static str,
) {
    if let Some(import) = imports_by_file.get(&(source_file_id, name.to_owned())) {
        if let Some(target_file_id) = import.target_file_id {
            let target_name = import.imported_name.as_deref().unwrap_or(name);
            let symbol = symbols_by_file_name
                .get(&(target_file_id, target_name.to_owned()))
                .and_then(|symbols| symbols.first());
            return (
                Some(target_file_id),
                symbol.map(|symbol| symbol.id.clone()),
                if symbol.is_some() {
                    None
                } else {
                    Some(target_name.to_owned())
                },
                if symbol.is_some() { 1.0 } else { 0.7 },
                if symbol.is_some() { "semantic" } else { "ast" },
            );
        }
    }
    let symbol = symbols_by_file_name
        .get(&(source_file_id, name.to_owned()))
        .and_then(|symbols| symbols.first());
    (
        Some(source_file_id),
        symbol.map(|symbol| symbol.id.clone()),
        if symbol.is_some() {
            None
        } else {
            Some(name.to_owned())
        },
        if symbol.is_some() { 0.7 } else { 0.4 },
        if symbol.is_some() { "ast" } else { "heuristic" },
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_edge(
    transaction: &Transaction<'_>,
    source_file_id: i64,
    source_symbol_id: Option<&str>,
    target_file_id: Option<i64>,
    target_symbol_id: Option<&str>,
    target_external_name: Option<String>,
    edge_type: &str,
    confidence: f64,
    resolution_method: &str,
) -> Result<()> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO symbol_edges(source_file_id, source_symbol_id, target_file_id, target_symbol_id, target_external_name, edge_type, confidence, resolution_method, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            params![source_file_id, source_symbol_id, target_file_id, target_symbol_id, target_external_name, edge_type, confidence, resolution_method],
        )
        .map_err(database_error)?;
    Ok(())
}

fn path_base_name(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or(path)
        .to_owned()
}

fn test_base_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.strip_suffix(".test")
        .or_else(|| stem.strip_suffix(".spec"))
        .or_else(|| stem.strip_suffix("_test"))
        .unwrap_or(stem)
        .to_owned()
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower
        .split('/')
        .any(|part| part == "tests" || part == "__tests__")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.contains("_test.")
}

pub(crate) fn symbol_id(
    repository_name: &str,
    relative_path: &str,
    name: &str,
    start_byte: usize,
    index: usize,
) -> String {
    format!("symbol:{repository_name}:{relative_path}:{name}:{start_byte}:{index}")
}

fn initialize_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE files(id INTEGER PRIMARY KEY, relative_path TEXT NOT NULL UNIQUE, language TEXT NOT NULL, content_hash TEXT NOT NULL, public_api_hash TEXT NOT NULL, source TEXT NOT NULL, size_bytes INTEGER NOT NULL);
             CREATE TABLE symbols(id INTEGER PRIMARY KEY, symbol_id TEXT NOT NULL UNIQUE, file_id INTEGER NOT NULL REFERENCES files(id), name TEXT NOT NULL, qualified_name TEXT, kind TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL);
             CREATE TABLE references_index(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), name TEXT NOT NULL, target TEXT, kind TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL);
             CREATE TABLE imports(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), source TEXT NOT NULL, imported_name TEXT, local_name TEXT, resolved_path TEXT);
             CREATE TABLE exports(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), exported_name TEXT NOT NULL, local_name TEXT);
             CREATE TABLE calls(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), callee TEXT NOT NULL, start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL);
             CREATE TABLE diagnostics(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), severity TEXT NOT NULL, message TEXT NOT NULL);
             CREATE TABLE chunks(id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id), start_byte INTEGER NOT NULL, end_byte INTEGER NOT NULL, content TEXT NOT NULL);
             CREATE TABLE file_states(relative_path TEXT PRIMARY KEY, status TEXT NOT NULL, observed_hash TEXT, error TEXT, updated_at TEXT NOT NULL);
             CREATE TABLE symbol_edges(id INTEGER PRIMARY KEY, source_file_id INTEGER NOT NULL REFERENCES files(id), source_symbol_id TEXT, target_file_id INTEGER REFERENCES files(id), target_symbol_id TEXT, target_external_name TEXT, edge_type TEXT NOT NULL, confidence REAL NOT NULL, resolution_method TEXT NOT NULL, metadata_json TEXT, UNIQUE(source_file_id, source_symbol_id, target_file_id, target_symbol_id, target_external_name, edge_type));
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

fn metadata_value(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
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
