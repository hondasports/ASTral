use std::{fs, time::Duration};

use astral::{
    incremental::{FileStateStatus, IncrementalIndexer},
    index::{IndexStore, SymbolResult},
};
use tempfile::tempdir;

fn symbol_names(results: &[SymbolResult]) -> Vec<String> {
    results.iter().map(|result| result.name.clone()).collect()
}

#[test]
fn refreshes_modified_renamed_and_deleted_files() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    let source = repository.path().join("app.ts");
    let renamed = repository.path().join("renamed.ts");
    fs::write(&source, "export function before() { return 1; }\n").expect("source");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at("test-repo", repository.path(), &database).expect("initial index");
    let indexer = IncrementalIndexer::new("test-repo", repository.path(), &database);

    fs::write(&source, "export function after() { return 2; }\n").expect("modified source");
    let report = indexer.refresh().expect("modify refresh");
    assert_eq!(report.updated_files, 1);
    assert!(IndexStore::find_symbol_at(&database, "before")
        .expect("old symbol search")
        .is_empty());
    assert_eq!(
        symbol_names(&IndexStore::find_symbol_at(&database, "after").expect("new symbol search")),
        vec!["after".to_owned()]
    );

    fs::rename(&source, &renamed).expect("rename source");
    let report = indexer.refresh().expect("rename refresh");
    assert_eq!(report.updated_files, 1);
    assert_eq!(report.removed_files, 1);
    assert!(IndexStore::find_symbol_at(&database, "after")
        .expect("renamed symbol search")
        .iter()
        .all(|result| result.relative_path == "renamed.ts"));

    fs::remove_file(&renamed).expect("delete source");
    let report = indexer.refresh().expect("delete refresh");
    assert_eq!(report.removed_files, 1);
    assert!(IndexStore::find_symbol_at(&database, "after")
        .expect("deleted symbol search")
        .is_empty());
}

#[test]
fn keeps_last_good_data_out_of_search_while_a_file_is_stale() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    let source = repository.path().join("app.ts");
    fs::write(&source, "export function stable() { return 1; }\n").expect("source");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at("test-repo", repository.path(), &database).expect("initial index");
    let indexer = IncrementalIndexer::new("test-repo", repository.path(), &database);

    fs::write(&source, "export function broken( {\n").expect("broken source");
    let report = indexer.refresh().expect("stale refresh");
    assert_eq!(report.stale_files, 1);
    assert!(IndexStore::find_symbol_at(&database, "stable")
        .expect("stale symbol search")
        .is_empty());
    assert_eq!(
        IndexStore::file_state_at(&database, "app.ts")
            .expect("file state")
            .expect("state exists")
            .status,
        FileStateStatus::Stale
    );

    fs::write(&source, "export function recovered() { return 2; }\n").expect("recovered source");
    let report = indexer.refresh().expect("recovery refresh");
    assert_eq!(report.updated_files, 1);
    assert_eq!(
        IndexStore::file_state_at(&database, "app.ts")
            .expect("file state")
            .expect("state exists")
            .status,
        FileStateStatus::Fresh
    );
}

#[test]
fn debounce_and_batch_windows_are_deterministic() {
    let mut batcher =
        astral::incremental::DirtyBatcher::new(Duration::from_millis(500), Duration::from_secs(1));
    let start = std::time::Instant::now();
    batcher.record("app.ts".into(), start);
    assert!(!batcher.ready(start + Duration::from_millis(499)));
    assert!(batcher.ready(start + Duration::from_millis(500)));
    let paths = batcher.take(start + Duration::from_millis(500));
    assert_eq!(paths, vec![std::path::PathBuf::from("app.ts")]);
}
