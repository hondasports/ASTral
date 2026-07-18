#[derive(Debug, Clone, PartialEq)]
pub struct RankingExplanation {
    pub score: f64,
    pub matched_by: Vec<String>,
    pub reason: String,
}

pub fn tokenize(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in query.split(|character: char| !character.is_alphanumeric() && character != '_') {
        if word.is_empty() {
            continue;
        }
        let mut start = 0;
        let bytes = word.as_bytes();
        for index in 1..bytes.len() {
            if bytes[index].is_ascii_uppercase()
                && !bytes[index - 1].is_ascii_uppercase()
                && index > start
            {
                tokens.push(word[start..index].to_ascii_lowercase());
                start = index;
            }
        }
        tokens.push(word[start..].replace('_', " ").to_ascii_lowercase());
    }
    tokens
        .into_iter()
        .flat_map(|token| {
            token
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|token| token.len() > 1)
        .collect()
}

pub fn fts_query(query: &str) -> String {
    tokenize(query)
        .into_iter()
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

pub fn explain(
    query: &str,
    path: &str,
    content: &str,
    fts_rank: f64,
    graph_degree: usize,
) -> RankingExplanation {
    let tokens = tokenize(query);
    let lower_path = path.to_ascii_lowercase();
    let lower_content = content.to_ascii_lowercase();
    let path_matches = tokens
        .iter()
        .filter(|token| lower_path.contains(token.as_str()))
        .count();
    let content_matches = tokens
        .iter()
        .filter(|token| lower_content.contains(token.as_str()))
        .count();
    let mut matched_by = vec!["lexical".to_owned()];
    let mut score = 1.0 / (1.0 + fts_rank.abs());
    if path_matches > 0 {
        score += 0.25 * path_matches as f64 / tokens.len().max(1) as f64;
        matched_by.push("path".to_owned());
    }
    if content_matches > 0 {
        score += 0.05 * content_matches as f64 / tokens.len().max(1) as f64;
    }
    if graph_degree > 0 {
        score += 0.1 * graph_degree.min(10) as f64 / 10.0;
        matched_by.push("graph".to_owned());
    }
    RankingExplanation {
        score,
        reason: format!(
            "FTS rank、{} token、path近接{}件、graph接続{}件を統合",
            content_matches, path_matches, graph_degree
        ),
        matched_by,
    }
}

#[cfg(test)]
mod tests {
    use super::{explain, fts_query, tokenize};

    #[test]
    fn tokenizes_camel_case_and_snake_case() {
        assert_eq!(
            tokenize("createUser_session"),
            vec!["create", "user", "session"]
        );
        assert_eq!(fts_query("createUser"), "create* OR user*");
    }

    #[test]
    fn path_proximity_increases_score_and_explanation() {
        let result = explain("auth", "src/auth/session.ts", "create auth", -1.0, 2);
        assert!(result.score > 1.0 / 2.0);
        assert!(result.matched_by.iter().any(|value| value == "path"));
        assert!(result.matched_by.iter().any(|value| value == "graph"));
    }
}
