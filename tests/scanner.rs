use std::fs;

use astral::scanner::{SourceFile, SourceScanner};
use tempfile::tempdir;

#[test]
fn scanner_honors_gitignore_and_excludes_generated_and_secret_files() {
    let directory = tempdir().expect("temporary repository");
    fs::create_dir(directory.path().join(".git")).expect("git directory");
    fs::write(directory.path().join(".gitignore"), "ignored/\n").expect("gitignore");
    fs::create_dir(directory.path().join("ignored")).expect("ignored directory");
    fs::create_dir(directory.path().join("node_modules")).expect("node_modules directory");
    fs::write(directory.path().join("src.ts"), "export const answer = 42;").expect("source");
    fs::write(
        directory.path().join("component.tsx"),
        "export const App = () => null;",
    )
    .expect("tsx source");
    fs::write(
        directory.path().join("ignored").join("file.ts"),
        "const ignored = true;",
    )
    .expect("ignored source");
    fs::write(
        directory.path().join("node_modules").join("package.js"),
        "module.exports = {};",
    )
    .expect("dependency source");
    fs::write(directory.path().join("bundle.min.js"), "generated();").expect("generated source");
    fs::write(directory.path().join(".env"), "TOKEN=secret").expect("secret file");

    let files = SourceScanner::new(directory.path())
        .scan()
        .expect("scan succeeds");
    let paths: Vec<_> = files
        .iter()
        .map(|file| file.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();

    assert_eq!(paths, vec!["component.tsx", "src.ts"]);
    assert_eq!(files[0].language, "tsx");
    assert!(matches!(&files[0], SourceFile { .. }));
}

#[test]
fn scanner_returns_supported_source_types_only() {
    let directory = tempdir().expect("temporary repository");
    fs::create_dir(directory.path().join(".git")).expect("git directory");
    for name in [
        "a.js", "b.jsx", "c.ts", "d.tsx", "e.mjs", "f.cjs", "g.mts", "h.cts",
    ] {
        fs::write(directory.path().join(name), "const value = 1;").expect("source");
    }
    fs::write(directory.path().join("README.md"), "not source").expect("markdown");

    let files = SourceScanner::new(directory.path())
        .scan()
        .expect("scan succeeds");
    assert_eq!(files.len(), 8);
}
