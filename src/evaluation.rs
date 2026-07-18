use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{AstralError, Result},
    index::IndexStore,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluationCase {
    pub id: String,
    pub query: String,
    pub expected_paths: Vec<String>,
    pub k: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluationCaseResult {
    pub id: String,
    pub query: String,
    pub precision_at_k: f64,
    pub recall_at_k: f64,
    pub reciprocal_rank: f64,
    pub hits: usize,
    pub top_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluationReport {
    pub cases: usize,
    pub mean_precision_at_k: f64,
    pub mean_recall_at_k: f64,
    pub mean_reciprocal_rank: f64,
    pub results: Vec<EvaluationCaseResult>,
}

pub fn evaluate(root: &Path, dataset: &Path) -> Result<EvaluationReport> {
    let source = fs::read_to_string(dataset).map_err(|source| AstralError::PathAccess {
        path: dataset.to_path_buf(),
        source,
    })?;
    let cases: Vec<EvaluationCase> =
        serde_json::from_str(&source).map_err(|error| AstralError::InvalidConfiguration {
            message: format!("invalid evaluation dataset: {error}"),
        })?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        let k = case.k.max(1);
        let search = IndexStore::search_code(root, &case.query)?;
        let mut seen_paths = HashSet::new();
        let top_paths: Vec<_> = search
            .iter()
            .filter(|result| seen_paths.insert(result.relative_path.clone()))
            .map(|result| result.relative_path.clone())
            .take(k)
            .collect();
        let expected: HashSet<_> = case.expected_paths.iter().cloned().collect();
        let hits = top_paths
            .iter()
            .filter(|path| expected.contains(*path))
            .count();
        let precision_at_k = hits as f64 / k as f64;
        let recall_at_k = if expected.is_empty() {
            0.0
        } else {
            hits as f64 / expected.len() as f64
        };
        let reciprocal_rank = top_paths
            .iter()
            .position(|path| expected.contains(path))
            .map_or(0.0, |position| 1.0 / (position + 1) as f64);
        results.push(EvaluationCaseResult {
            id: case.id,
            query: case.query,
            precision_at_k,
            recall_at_k,
            reciprocal_rank,
            hits,
            top_paths,
        });
    }
    let cases = results.len();
    let divisor = cases.max(1) as f64;
    Ok(EvaluationReport {
        cases,
        mean_precision_at_k: results
            .iter()
            .map(|result| result.precision_at_k)
            .sum::<f64>()
            / divisor,
        mean_recall_at_k: results.iter().map(|result| result.recall_at_k).sum::<f64>() / divisor,
        mean_reciprocal_rank: results
            .iter()
            .map(|result| result.reciprocal_rank)
            .sum::<f64>()
            / divisor,
        results,
    })
}

pub fn default_dataset(root: &Path) -> PathBuf {
    root.join("evaluation").join("search_quality.json")
}
