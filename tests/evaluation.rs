use std::fs;

use astral::evaluation::evaluate;
use tempfile::tempdir;

#[test]
fn evaluates_expected_paths_and_recall() {
    let repository = tempdir().expect("temporary repository");
    fs::create_dir(repository.path().join(".git")).expect("git directory");
    fs::write(
        repository.path().join("target.ts"),
        "export function refresh_stale_file() { return 'stale file'; }\n",
    )
    .expect("source");
    let dataset = repository.path().join("dataset.json");
    fs::write(
        &dataset,
        r#"[
          {
            "id": "stale",
            "query": "stale file",
            "expected_paths": ["target.ts"],
            "k": 5
          }
        ]"#,
    )
    .expect("dataset");
    std::env::set_var("ASTRAL_DATA_DIR", repository.path().join(".astral-data"));

    let report = evaluate(repository.path(), &dataset).expect("evaluation succeeds");
    assert_eq!(report.cases, 1);
    assert_eq!(report.mean_recall_at_k, 1.0);
    assert_eq!(report.results[0].top_paths, vec!["target.ts"]);
}
