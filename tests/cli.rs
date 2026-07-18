use std::{fs, process::Command};

use tempfile::tempdir;

#[test]
fn help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_astral"))
        .arg("--help")
        .output()
        .expect("run astral help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AST-aware repository context engine"));
    assert!(stdout.contains("status"));
}

#[test]
fn status_resolves_a_nested_path_and_reports_not_indexed() {
    let directory = tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join(".git")).expect("git metadata");
    let nested = directory.path().join("src");
    fs::create_dir(&nested).expect("nested directory");

    let output = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["status", nested.to_str().expect("UTF-8 path")])
        .output()
        .expect("run astral status");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Repository root:"));
    assert!(stdout.contains("Index status: not indexed"));
}

#[test]
fn status_rejects_a_path_without_a_repository_root() {
    let directory = tempdir().expect("temporary directory");

    let output = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["status", directory.path().to_str().expect("UTF-8 path")])
        .output()
        .expect("run astral status");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("repository root not found"));
}

#[test]
fn status_rejects_a_missing_path() {
    let directory = tempdir().expect("temporary directory");
    let missing = directory.path().join("missing");

    let output = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["status", missing.to_str().expect("UTF-8 path")])
        .output()
        .expect("run astral status");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("path does not exist"));
}

#[test]
fn index_and_search_commands_work_for_a_typescript_repository() {
    let directory = tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join(".git")).expect("git metadata");
    fs::write(
        directory.path().join("app.ts"),
        "export function App() { return 42; }\n",
    )
    .expect("source");
    let path = directory.path().to_str().expect("UTF-8 path");
    let data_dir = directory.path().join(".astral-data");

    let index = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["index", path])
        .env("ASTRAL_DATA_DIR", &data_dir)
        .output()
        .expect("run astral index");
    assert!(index.status.success());

    let search = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["search-code", path, "App"])
        .env("ASTRAL_DATA_DIR", &data_dir)
        .output()
        .expect("run astral search-code");
    assert!(search.status.success());
    assert!(String::from_utf8_lossy(&search.stdout).contains("app.ts"));

    let symbols = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["find-symbol", path, "App"])
        .env("ASTRAL_DATA_DIR", &data_dir)
        .output()
        .expect("run astral find-symbol");
    assert!(symbols.status.success());
    let symbol_line = String::from_utf8_lossy(&symbols.stdout);
    let symbol_id = symbol_line.split_whitespace().next().expect("symbol id");

    let read = Command::new(env!("CARGO_BIN_EXE_astral"))
        .args(["read-symbol", path, symbol_id])
        .env("ASTRAL_DATA_DIR", &data_dir)
        .output()
        .expect("run astral read-symbol");
    assert!(read.status.success());
    assert!(String::from_utf8_lossy(&read.stdout).contains("function App"));
}
