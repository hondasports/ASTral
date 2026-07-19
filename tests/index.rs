use std::fs;

use astral::index::{IndexStore, SearchResult};
use rusqlite::Connection;
use tempfile::{tempdir, TempDir};

#[test]
fn rebuilds_sqlite_index_and_supports_code_symbol_and_symbol_read_searches() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    fs::write(
        repository.path().join("value.ts"),
        "export const value = 42;\n",
    )
    .expect("value source");
    fs::write(
        repository.path().join("app.tsx"),
        "import { value } from './value';\nexport function App() { return value; }\n",
    )
    .expect("app source");
    let database = repository.path().join("index.sqlite");

    let status = IndexStore::rebuild_at("test-repo", repository.path(), &database)
        .expect("rebuild succeeds");
    assert!(status.indexed);
    assert_eq!(status.file_count, 2);
    assert!(status.symbol_count >= 2);

    let code = IndexStore::search_code_at(&database, "value").expect("code search succeeds");
    assert!(code.iter().any(|result| result.relative_path == "value.ts"));

    let symbols = IndexStore::find_symbol_at(&database, "App").expect("symbol search succeeds");
    let app = symbols
        .iter()
        .find(|symbol| symbol.name == "App")
        .expect("App symbol");
    let read = IndexStore::read_symbol_at(&database, &app.symbol_id).expect("symbol read succeeds");
    assert!(read.source.contains("function App"));

    let status = IndexStore::status_at(&database).expect("status succeeds");
    assert_eq!(status.schema_version, 5);
}

#[test]
fn failed_rebuild_keeps_the_active_database_usable() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    let source = repository.path().join("app.ts");
    fs::write(&source, "export const stable = 1;\n").expect("source");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at("test-repo", repository.path(), &database)
        .expect("initial rebuild succeeds");

    fs::write(&source, [0xff, 0xfe]).expect("invalid source");
    assert!(IndexStore::rebuild_at("test-repo", repository.path(), &database).is_err());

    let result = IndexStore::find_symbol_at(&database, "stable").expect("old index remains");
    assert!(result.iter().any(|symbol| symbol.name == "stable"));
}

#[test]
fn same_symbol_name_is_isolated_by_repository_database_and_id() {
    let first = repository_with_app("first");
    let second = repository_with_app("second");
    let first_database = first.path().join("index.sqlite");
    let second_database = second.path().join("index.sqlite");

    IndexStore::rebuild_at("repo-one", first.path(), &first_database)
        .expect("first rebuild succeeds");
    IndexStore::rebuild_at("repo-two", second.path(), &second_database)
        .expect("second rebuild succeeds");

    let first_symbol = IndexStore::find_symbol_at(&first_database, "App")
        .expect("first symbol search succeeds")
        .into_iter()
        .next()
        .expect("first App symbol");
    let second_symbol = IndexStore::find_symbol_at(&second_database, "App")
        .expect("second symbol search succeeds")
        .into_iter()
        .next()
        .expect("second App symbol");

    assert!(first_symbol.symbol_id.starts_with("symbol:repo-one:"));
    assert!(second_symbol.symbol_id.starts_with("symbol:repo-two:"));
    assert_ne!(first_symbol.symbol_id, second_symbol.symbol_id);
    assert!(IndexStore::read_symbol_at(&second_database, &first_symbol.symbol_id).is_err());
}

#[test]
fn schema_four_index_is_rebuilt_with_namespace_ids() {
    let repository = repository_with_app("legacy");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at("repo-one", repository.path(), &database)
        .expect("initial rebuild succeeds");
    let connection = Connection::open(&database).expect("open index database");
    connection
        .execute(
            "UPDATE metadata SET value = '4' WHERE key = 'schema_version'",
            [],
        )
        .expect("mark legacy schema");
    drop(connection);

    let report =
        astral::incremental::IncrementalIndexer::new("repo-one", repository.path(), &database)
            .refresh()
            .expect("legacy index refresh succeeds");

    assert!(report.rebuilt);
    assert_eq!(
        IndexStore::status_at(&database)
            .expect("status succeeds")
            .schema_version,
        5
    );
    assert!(IndexStore::find_symbol_at(&database, "App")
        .expect("symbol search succeeds")
        .iter()
        .all(|symbol| symbol.symbol_id.starts_with("symbol:repo-one:")));
}

#[test]
fn changed_repository_name_rebuilds_the_existing_index_namespace() {
    let repository = repository_with_app("renamed");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at("old-name", repository.path(), &database)
        .expect("initial rebuild succeeds");

    let report =
        astral::incremental::IncrementalIndexer::new("new-name", repository.path(), &database)
            .refresh()
            .expect("renamed index refresh succeeds");

    assert!(report.rebuilt);
    assert!(IndexStore::find_symbol_at(&database, "App")
        .expect("symbol search succeeds")
        .iter()
        .all(|symbol| symbol.symbol_id.starts_with("symbol:new-name:")));
}

fn repository_with_app(value: &str) -> TempDir {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    fs::write(
        repository.path().join("app.ts"),
        format!("export function App() {{ return '{value}'; }}\n"),
    )
    .expect("app source");
    repository
}

#[allow(dead_code)]
fn _assert_search_result_shape(result: &SearchResult) {
    assert!(!result.relative_path.is_empty());
}
