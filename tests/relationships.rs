use std::fs;

use astral::index::IndexStore;
use tempfile::tempdir;

#[test]
fn finds_references_callers_callees_and_related_tests() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    fs::create_dir(repository.path().join("tests")).expect("tests directory");
    fs::write(
        repository.path().join("value.ts"),
        "export const value = 1;\n",
    )
    .expect("value source");
    fs::write(
        repository.path().join("app.ts"),
        "import { value } from './value';\nexport function helper() { return value; }\nexport function create() { return helper(); }\n",
    )
    .expect("app source");
    fs::write(
        repository.path().join("tests/app.test.ts"),
        "import { create } from '../app';\ncreate();\n",
    )
    .expect("test source");
    let database = repository.path().join("index.sqlite");
    IndexStore::rebuild_at(repository.path(), &database).expect("initial index");

    let callers = IndexStore::find_callers(repository.path(), "create").expect("callers");
    assert!(callers
        .iter()
        .any(|result| result.source_file == "tests/app.test.ts"));
    assert!(callers.iter().all(|result| result.confidence > 0.0));

    let callees = IndexStore::find_callees(repository.path(), "create").expect("callees");
    assert!(callees
        .iter()
        .any(|result| result.target_name.as_deref() == Some("helper")));

    let references = IndexStore::find_references(repository.path(), "value").expect("references");
    assert!(references
        .iter()
        .any(|result| result.source_file == "app.ts"));

    let related_tests =
        IndexStore::find_related_tests(repository.path(), "create").expect("related tests");
    assert!(related_tests
        .iter()
        .any(|result| result.source_file == "tests/app.test.ts"));
}
