use std::{fs, path::Path};

use astral::{
    analyzer::{LanguageAnalyzer, ReferenceKind, SymbolKind},
    oxc_analyzer::OxcAnalyzer,
};
use tempfile::tempdir;

#[test]
fn extracts_javascript_symbols_references_imports_exports_and_calls() {
    let source = r#"
import { value as imported } from './value.js';
export function create(value) {
  return imported(value);
}
export class Service {}
const App = () => create(1);
"#;

    let result = OxcAnalyzer::default()
        .analyze(Path::new("src/app.jsx"), source)
        .expect("analysis succeeds");

    assert!(result
        .symbols
        .iter()
        .any(|symbol| { symbol.name == "create" && symbol.kind == SymbolKind::Function }));
    assert!(result
        .symbols
        .iter()
        .any(|symbol| { symbol.name == "Service" && symbol.kind == SymbolKind::Class }));
    assert!(result
        .symbols
        .iter()
        .any(|symbol| { symbol.name == "App" && symbol.kind == SymbolKind::Function }));
    let scoped_functions: Vec<_> = OxcAnalyzer::default()
        .analyze(
            Path::new("scope.ts"),
            "function same() {} { function same() {} }",
        )
        .expect("scoped analysis succeeds")
        .symbols
        .into_iter()
        .filter(|symbol| symbol.name == "same")
        .filter_map(|symbol| symbol.qualified_name)
        .collect();
    assert_eq!(scoped_functions.len(), 2);
    assert_ne!(scoped_functions[0], scoped_functions[1]);
    assert!(result.imports.iter().any(|import| {
        import.source == "./value.js"
            && import.imported_name.as_deref() == Some("value")
            && import.local_name.as_deref() == Some("imported")
    }));
    assert!(result
        .exports
        .iter()
        .any(|export| export.exported_name == "create"));
    assert!(result.calls.iter().any(|call| call.callee == "imported"));
    assert!(result.references.iter().any(|reference| {
        reference.name == "imported"
            && reference.target.as_deref() == Some("imported")
            && reference.kind == ReferenceKind::Call
    }));
}

#[test]
fn extracts_typescript_declarations_and_keeps_diagnostics_without_panicking() {
    let source = r#"
interface User { id: string }
type UserId = User['id'];
function load(id: UserId): User { return { id }; }
"#;
    let result = OxcAnalyzer::default()
        .analyze(Path::new("src/user.ts"), source)
        .expect("analysis succeeds");

    assert!(result
        .symbols
        .iter()
        .any(|symbol| symbol.name == "User" && symbol.kind == SymbolKind::Interface));
    assert!(result
        .symbols
        .iter()
        .any(|symbol| symbol.name == "UserId" && symbol.kind == SymbolKind::Type));
    assert!(result.diagnostics.is_empty());

    let invalid = OxcAnalyzer::default()
        .analyze(Path::new("broken.ts"), "function broken( {")
        .expect("diagnostic result is not a fatal analyzer error");
    assert!(!invalid.diagnostics.is_empty());
}

#[test]
fn resolves_relative_imports_with_the_repository_resolver() {
    let repository = tempdir().expect("temporary repository");
    fs::write(
        repository.path().join("value.ts"),
        "export const value = 1;",
    )
    .expect("value");
    let analyzer = OxcAnalyzer::new(repository.path());
    let result = analyzer
        .analyze(Path::new("app.ts"), "import { value } from './value';")
        .expect("analysis succeeds");

    assert_eq!(
        result.imports[0].resolved_path.as_deref(),
        Some(Path::new("value.ts"))
    );
}
