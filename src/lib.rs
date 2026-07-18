pub mod analyzer;
pub mod config;
pub mod error;
pub mod incremental;
pub mod index;
pub mod logging;
pub mod mcp;
pub mod oxc_analyzer;
pub mod repository;
pub mod scanner;

pub use analyzer::{
    AnalysisResult, AnalyzerMetadata, Call, Diagnostic, DiagnosticSeverity, Export, Import,
    LanguageAnalyzer, Reference, ReferenceKind, SourceRange, Symbol, SymbolKind,
};
pub use error::{AstralError, Result};
pub use repository::RepositoryRoot;
