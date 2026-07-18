use std::{fs, process::Command};

use astral::git;
use tempfile::tempdir;

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn detects_head_branch_and_working_tree_changes() {
    let repository = tempdir().expect("temporary repository");
    run_git(repository.path(), &["init"]);
    run_git(
        repository.path(),
        &["config", "user.email", "test@example.com"],
    );
    run_git(repository.path(), &["config", "user.name", "ASTral Test"]);
    fs::write(repository.path().join("app.ts"), "export const app = 1;\n").expect("source");
    run_git(repository.path(), &["add", "app.ts"]);
    run_git(repository.path(), &["commit", "-m", "initial"]);

    let clean = git::inspect(repository.path()).expect("git inspection");
    assert!(clean.available);
    assert!(clean.head.is_some());
    assert!(clean.branch.is_some());
    assert!(!clean.dirty);

    run_git(repository.path(), &["checkout", "--detach"]);
    let detached = git::inspect(repository.path()).expect("detached HEAD inspection");
    assert!(detached.available);
    assert!(detached.head.is_some());
    assert!(detached.branch.is_none());

    fs::write(repository.path().join("app.ts"), "export const app = 2;\n").expect("modified");
    let dirty = git::inspect(repository.path()).expect("git inspection");
    assert!(dirty.dirty);
    assert!(dirty.dirty_file_count >= 1);
    assert_ne!(dirty.worktree_hash, clean.worktree_hash);
}
