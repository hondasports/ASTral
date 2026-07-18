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
