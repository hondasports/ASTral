use std::path::{Path, PathBuf};

use crate::error::Result;

pub trait LanguageAnalyzer {
    fn supports(&self, path: &Path) -> bool;
    fn analyze(&self, path: &Path, source: &str) -> Result<AnalysisResult>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResult {
    pub path: PathBuf,
    pub analyzer: AnalyzerMetadata,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub calls: Vec<Call>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerMetadata {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub qualified_name: Option<String>,
    pub kind: SymbolKind,
    pub range: SourceRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    Type,
    Variable,
    Module,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub target: Option<String>,
    pub kind: ReferenceKind,
    pub range: SourceRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Read,
    Write,
    Call,
    Import,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub source: String,
    pub imported_name: Option<String>,
    pub local_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub exported_name: String,
    pub local_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub callee: String,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{AnalysisResult, AnalyzerMetadata, LanguageAnalyzer};
    use crate::Result;

    struct FixtureAnalyzer;

    impl LanguageAnalyzer for FixtureAnalyzer {
        fn supports(&self, path: &Path) -> bool {
            path.extension()
                .is_some_and(|extension| extension == "fixture")
        }

        fn analyze(&self, path: &Path, _source: &str) -> Result<AnalysisResult> {
            Ok(AnalysisResult {
                path: path.to_path_buf(),
                analyzer: AnalyzerMetadata {
                    name: "fixture".to_owned(),
                    version: "0.1".to_owned(),
                },
                symbols: Vec::new(),
                references: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                calls: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
    }

    #[test]
    fn analyzer_contract_uses_normalized_owned_data() {
        let analyzer = FixtureAnalyzer;
        let result = analyzer
            .analyze(Path::new("example.fixture"), "source")
            .expect("analysis result");

        assert!(analyzer.supports(Path::new("example.fixture")));
        assert_eq!(result.analyzer.name, "fixture");
        assert!(result.symbols.is_empty());
    }
}
