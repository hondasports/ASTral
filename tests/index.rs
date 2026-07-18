use std::fs;

use astral::index::{IndexStore, SearchResult};
use tempfile::tempdir;

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

    let status = IndexStore::rebuild_at(repository.path(), &database).expect("rebuild succeeds");
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
    assert_eq!(status.schema_version, 4);
}

#[test]
fn failed_rebuild_keeps_the_active_database_usable() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    let source = repository.path().join("app.ts");
    fs::write(&source, "export const stable = 1;\n").expect("source");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at(repository.path(), &database).expect("initial rebuild succeeds");

    fs::write(&source, [0xff, 0xfe]).expect("invalid source");
    assert!(IndexStore::rebuild_at(repository.path(), &database).is_err());

    let result = IndexStore::find_symbol_at(&database, "stable").expect("old index remains");
    assert!(result.iter().any(|symbol| symbol.name == "stable"));
}

#[allow(dead_code)]
fn _assert_search_result_shape(result: &SearchResult) {
    assert!(!result.relative_path.is_empty());
}
