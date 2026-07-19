use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    time::{Duration, Instant},
};

use notify::{Event, EventKind, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::{
    analyzer::{AnalysisResult, LanguageAnalyzer},
    error::{AstralError, Result},
    index::{
        database_error, file_state_name, hash_bytes, now_string, public_api_hash, rebuild_edges,
        symbol_id, symbol_kind_name, IndexStore, SCHEMA_VERSION,
    },
    oxc_analyzer::OxcAnalyzer,
    scanner::{SourceFile, SourceScanner},
};

pub use crate::index::FileStateStatus;

const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(500);
const DEFAULT_BATCH_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshReport {
    pub updated_files: usize,
    pub stale_files: usize,
    pub removed_files: usize,
    pub rebuilt: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct FileUpdate {
    indexed: bool,
    api_changed: bool,
}

#[derive(Debug, Clone)]
pub struct IncrementalIndexer {
    repository_name: String,
    root: PathBuf,
    database_path: PathBuf,
    debounce: Duration,
    batch_window: Duration,
}

impl IncrementalIndexer {
    pub fn new(
        repository_name: impl Into<String>,
        root: impl Into<PathBuf>,
        database_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repository_name: repository_name.into(),
            root: root.into(),
            database_path: database_path.into(),
            debounce: DEFAULT_DEBOUNCE,
            batch_window: DEFAULT_BATCH_WINDOW,
        }
    }

    pub fn with_timing(mut self, debounce: Duration, batch_window: Duration) -> Self {
        self.debounce = debounce;
        self.batch_window = batch_window;
        self
    }

    pub fn refresh(&self) -> Result<RefreshReport> {
        let status = IndexStore::status_at(&self.database_path)?;
        let indexed_repository_name = IndexStore::repository_name_at(&self.database_path)?;
        if !status.indexed
            || status.schema_version != SCHEMA_VERSION
            || indexed_repository_name.as_deref() != Some(&self.repository_name)
        {
            IndexStore::rebuild_at(&self.repository_name, &self.root, &self.database_path)?;
            return Ok(RefreshReport {
                rebuilt: true,
                ..RefreshReport::default()
            });
        }
        let git = crate::git::inspect(&self.root)?;
        if git.available && git_head_changed(&self.database_path, &git)? {
            IndexStore::rebuild_at(&self.repository_name, &self.root, &self.database_path)?;
            return Ok(RefreshReport {
                rebuilt: true,
                ..RefreshReport::default()
            });
        }

        let files = SourceScanner::new(&self.root).scan()?;
        let current: HashMap<String, SourceFile> = files
            .into_iter()
            .map(|file| (relative_path(&file.relative_path), file))
            .collect();
        let indexed = indexed_files(&self.database_path)?;
        let analyzer = OxcAnalyzer::new(&self.root);
        let mut report = RefreshReport::default();
        let mut updated_paths = HashSet::new();
        let mut api_changed_paths = Vec::new();

        for (path, file) in &current {
            let hash = hash_bytes(file.source.as_bytes());
            let state = IndexStore::file_state_at(&self.database_path, path)?
                .map(|state| state.status)
                .unwrap_or(FileStateStatus::Stale);
            if indexed.get(path) != Some(&hash) || state != FileStateStatus::Fresh {
                let update =
                    update_file(&self.repository_name, &self.database_path, file, &analyzer)?;
                updated_paths.insert(path.clone());
                if update.indexed {
                    report.updated_files += 1;
                    if update.api_changed {
                        api_changed_paths.push(path.clone());
                    }
                } else {
                    report.stale_files += 1;
                }
            }
        }

        let removed_paths: Vec<_> = indexed
            .keys()
            .filter(|path| !current.contains_key(*path))
            .cloned()
            .collect();
        for path in &removed_paths {
            remove_file(&self.database_path, path)?;
            report.removed_files += 1;
        }
        let mut dependent_paths = HashSet::new();
        for path in api_changed_paths.iter().chain(&removed_paths) {
            dependent_paths.extend(importer_paths(&self.database_path, path)?);
        }
        for path in dependent_paths {
            if updated_paths.contains(&path) {
                continue;
            }
            if let Some(file) = current.get(&path) {
                let update =
                    update_file(&self.repository_name, &self.database_path, file, &analyzer)?;
                if update.indexed {
                    report.updated_files += 1;
                } else {
                    report.stale_files += 1;
                }
            }
        }
        record_git_state(&self.database_path, &git)?;
        Ok(report)
    }

    pub fn watch(&self) -> Result<()> {
        let (sender, receiver) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(|error| AstralError::Indexing {
            message: error.to_string(),
        })?;
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|error| AstralError::Indexing {
                message: error.to_string(),
            })?;

        let mut batcher = DirtyBatcher::new(self.debounce, self.batch_window);
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(event)) => record_event(&mut batcher, event, Instant::now()),
                Ok(Err(error)) => {
                    return Err(AstralError::Indexing {
                        message: error.to_string(),
                    });
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(AstralError::Indexing {
                        message: "watcher channel disconnected".to_owned(),
                    });
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            if batcher.ready(now) {
                let dirty_paths = batcher.take(now);
                let started_at = Instant::now();
                tracing::info!(paths = dirty_paths.len(), "index refresh started");
                let report = self.refresh()?;
                tracing::info!(
                    updated_files = report.updated_files,
                    stale_files = report.stale_files,
                    removed_files = report.removed_files,
                    rebuilt = report.rebuilt,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "index refresh completed"
                );
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DirtyBatcher {
    paths: HashSet<PathBuf>,
    first_event: Option<Instant>,
    last_event: Option<Instant>,
    debounce: Duration,
    batch_window: Duration,
}

impl DirtyBatcher {
    pub fn new(debounce: Duration, batch_window: Duration) -> Self {
        Self {
            debounce,
            batch_window,
            ..Self::default()
        }
    }

    pub fn record(&mut self, path: PathBuf, now: Instant) {
        self.paths.insert(path);
        self.first_event.get_or_insert(now);
        self.last_event = Some(now);
    }

    pub fn ready(&self, now: Instant) -> bool {
        let Some(first_event) = self.first_event else {
            return false;
        };
        let last_event = self
            .last_event
            .expect("first event always has a last event");
        now.duration_since(last_event) >= self.debounce
            || now.duration_since(first_event) >= self.batch_window
    }

    pub fn take(&mut self, _now: Instant) -> Vec<PathBuf> {
        self.first_event = None;
        self.last_event = None;
        let mut paths: Vec<_> = self.paths.drain().collect();
        paths.sort();
        paths
    }
}

fn record_event(batcher: &mut DirtyBatcher, event: Event, now: Instant) {
    if matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        for path in event.paths {
            batcher.record(path, now);
        }
    }
}

fn indexed_files(database_path: &Path) -> Result<HashMap<String, String>> {
    let connection = Connection::open(database_path).map_err(database_error)?;
    let mut statement = connection
        .prepare("SELECT relative_path, content_hash FROM files")
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(database_error)?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(database_error)
}

fn git_head_changed(database_path: &Path, git: &crate::git::GitSnapshot) -> Result<bool> {
    let connection = Connection::open(database_path).map_err(database_error)?;
    let stored = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'git_head'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(database_error)?;
    Ok(stored.as_deref() != git.head.as_deref())
}

fn record_git_state(database_path: &Path, git: &crate::git::GitSnapshot) -> Result<()> {
    let mut connection = Connection::open(database_path).map_err(database_error)?;
    let transaction = connection.transaction().map_err(database_error)?;
    for (key, value) in [
        ("git_available", git.available.to_string()),
        ("git_head", git.head.clone().unwrap_or_default()),
        ("git_branch", git.branch.clone().unwrap_or_default()),
        ("git_dirty", git.dirty.to_string()),
        ("git_dirty_file_count", git.dirty_file_count.to_string()),
        (
            "git_worktree_hash",
            git.worktree_hash.clone().unwrap_or_default(),
        ),
    ] {
        transaction
            .execute(
                "INSERT INTO metadata(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(database_error)?;
    }
    transaction.commit().map_err(database_error)
}

fn importer_paths(database_path: &Path, target_path: &str) -> Result<Vec<String>> {
    let connection = Connection::open(database_path).map_err(database_error)?;
    let mut statement = connection
        .prepare(
            "SELECT f.relative_path
             FROM imports i JOIN files f ON f.id = i.file_id
             WHERE REPLACE(i.resolved_path, char(92), '/') = ?1
             ORDER BY f.relative_path",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([target_path], |row| row.get(0))
        .map_err(database_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database_error)
}

fn update_file(
    repository_name: &str,
    database_path: &Path,
    file: &SourceFile,
    analyzer: &OxcAnalyzer,
) -> Result<FileUpdate> {
    let analysis = analyzer.analyze(&file.relative_path, &file.source)?;
    let path = relative_path(&file.relative_path);
    let hash = hash_bytes(file.source.as_bytes());
    let mut connection = Connection::open(database_path).map_err(database_error)?;
    let previous_api_hash = connection
        .query_row(
            "SELECT public_api_hash FROM files WHERE relative_path = ?1",
            [&path],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(database_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    if !analysis.diagnostics.is_empty() {
        upsert_state(
            &transaction,
            &path,
            FileStateStatus::Stale,
            Some(&hash),
            Some(&diagnostics_message(&analysis)),
        )?;
        transaction.commit().map_err(database_error)?;
        return Ok(FileUpdate::default());
    }
    delete_file_records(&transaction, &path)?;
    insert_analysis(&transaction, repository_name, file, &analysis, &path, &hash)?;
    upsert_state(
        &transaction,
        &path,
        FileStateStatus::Fresh,
        Some(&hash),
        None,
    )?;
    rebuild_edges(&transaction)?;
    let api_hash = public_api_hash(&analysis);
    transaction.commit().map_err(database_error)?;
    Ok(FileUpdate {
        indexed: true,
        api_changed: previous_api_hash.as_deref() != Some(api_hash.as_str()),
    })
}

fn remove_file(database_path: &Path, path: &str) -> Result<()> {
    let mut connection = Connection::open(database_path).map_err(database_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    delete_file_records(&transaction, path)?;
    upsert_state(&transaction, path, FileStateStatus::Missing, None, None)?;
    rebuild_edges(&transaction)?;
    transaction.commit().map_err(database_error)
}

fn delete_file_records(transaction: &Transaction<'_>, path: &str) -> Result<()> {
    let file_id = transaction
        .query_row(
            "SELECT id FROM files WHERE relative_path = ?1",
            [path],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(database_error)?;
    let Some(file_id) = file_id else {
        return Ok(());
    };
    transaction
        .execute(
            "DELETE FROM symbol_edges WHERE source_file_id = ?1 OR target_file_id = ?1",
            [file_id],
        )
        .map_err(database_error)?;
    transaction
        .execute(
            "DELETE FROM chunk_search WHERE rowid IN (SELECT id FROM chunks WHERE file_id = ?1)",
            [file_id],
        )
        .map_err(database_error)?;
    for table in [
        "symbols",
        "references_index",
        "imports",
        "exports",
        "calls",
        "diagnostics",
        "chunks",
    ] {
        transaction
            .execute(
                &format!("DELETE FROM {table} WHERE file_id = ?1"),
                [file_id],
            )
            .map_err(database_error)?;
    }
    transaction
        .execute("DELETE FROM files WHERE id = ?1", [file_id])
        .map_err(database_error)?;
    Ok(())
}

fn upsert_state(
    transaction: &Transaction<'_>,
    path: &str,
    status: FileStateStatus,
    hash: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO file_states(relative_path, status, observed_hash, error, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(relative_path) DO UPDATE SET status = excluded.status, observed_hash = excluded.observed_hash, error = excluded.error, updated_at = excluded.updated_at",
            params![path, file_state_name(status), hash, error, now_string()],
        )
        .map_err(database_error)?;
    Ok(())
}

fn insert_analysis(
    transaction: &Transaction<'_>,
    repository_name: &str,
    file: &SourceFile,
    analysis: &AnalysisResult,
    relative_path: &str,
    hash: &str,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO files(relative_path, language, content_hash, public_api_hash, source, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![relative_path, file.language, hash, public_api_hash(analysis), &file.source, file.source.len() as i64],
        )
        .map_err(database_error)?;
    let file_id = transaction.last_insert_rowid();
    for (index, symbol) in analysis.symbols.iter().enumerate() {
        let symbol_id = symbol_id(
            repository_name,
            relative_path,
            &symbol.name,
            symbol.range.start_byte,
            index,
        );
        transaction
            .execute(
                "INSERT INTO symbols(symbol_id, file_id, name, qualified_name, kind, start_byte, end_byte, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![symbol_id, file_id, symbol.name, symbol.qualified_name, symbol_kind_name(symbol.kind), symbol.range.start_byte as i64, symbol.range.end_byte as i64, symbol.range.start_line as i64, symbol.range.end_line as i64],
            )
            .map_err(database_error)?;
    }
    for reference in &analysis.references {
        transaction
            .execute(
                "INSERT INTO references_index(file_id, name, target, kind, start_byte, end_byte, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![file_id, reference.name, reference.target, format!("{:?}", reference.kind).to_lowercase(), reference.range.start_byte as i64, reference.range.end_byte as i64, reference.range.start_line as i64, reference.range.end_line as i64],
            )
            .map_err(database_error)?;
    }
    for import in &analysis.imports {
        transaction
            .execute(
                "INSERT INTO imports(file_id, source, imported_name, local_name, resolved_path) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![file_id, import.source, import.imported_name, import.local_name, import.resolved_path.as_ref().map(|path| path.to_string_lossy().to_string())],
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
                    call.range.end_byte as i64
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
    for (start, end) in crate::index::chunk_ranges(&file.source) {
        let content = &file.source[start..end];
        transaction
            .execute("INSERT INTO chunks(file_id, start_byte, end_byte, content) VALUES (?1, ?2, ?3, ?4)", params![file_id, start as i64, end as i64, content])
            .map_err(database_error)?;
        let chunk_id = transaction.last_insert_rowid();
        transaction
            .execute("INSERT INTO chunk_search(rowid, relative_path, start_byte, end_byte, content) VALUES (?1, ?2, ?3, ?4, ?5)", params![chunk_id, relative_path, start as i64, end as i64, content])
            .map_err(database_error)?;
    }
    Ok(())
}

fn diagnostics_message(analysis: &AnalysisResult) -> String {
    analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::DirtyBatcher;
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    #[test]
    fn batcher_deduplicates_paths_and_flushes_at_debounce() {
        let mut batcher = DirtyBatcher::new(Duration::from_millis(500), Duration::from_secs(1));
        let start = Instant::now();
        batcher.record(PathBuf::from("app.ts"), start);
        batcher.record(PathBuf::from("app.ts"), start);
        assert!(!batcher.ready(start + Duration::from_millis(499)));
        assert!(batcher.ready(start + Duration::from_millis(500)));
        assert_eq!(
            batcher.take(start + Duration::from_millis(500)),
            vec![PathBuf::from("app.ts")]
        );
    }
}
