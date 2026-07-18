use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use oxc_semantic::SemanticBuilder;
use oxc_span::{SourceType, Span};

use crate::{
    analyzer::{
        AnalysisResult, AnalyzerMetadata, Call, Diagnostic, DiagnosticSeverity, Export, Import,
        LanguageAnalyzer, Reference, ReferenceKind, SourceRange, Symbol, SymbolKind,
    },
    error::{AstralError, Result},
};

#[derive(Debug, Clone, Default)]
pub struct OxcAnalyzer {
    root: Option<std::path::PathBuf>,
}

impl OxcAnalyzer {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        let root = root.into();
        Self {
            root: Some(root.canonicalize().unwrap_or(root)),
        }
    }
}

impl LanguageAnalyzer for OxcAnalyzer {
    fn supports(&self, path: &Path) -> bool {
        crate::scanner::language_for(path).is_some()
    }

    fn analyze(&self, path: &Path, source: &str) -> Result<AnalysisResult> {
        let source_type =
            SourceType::from_path(path).map_err(|error| AstralError::InvalidConfiguration {
                message: error.to_string(),
            })?;
        let allocator = Allocator::default();
        let parser = Parser::new(&allocator, source, source_type).parse();
        let mut diagnostics = parser
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic_to_model(diagnostic.to_string()))
            .collect::<Vec<_>>();

        if parser.panicked {
            return Ok(empty_result(path, diagnostics));
        }

        let program = parser.program;
        let semantic = SemanticBuilder::new_compiler()
            .with_build_nodes(true)
            .build(&program);
        diagnostics.extend(
            semantic
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic_to_model(diagnostic.to_string())),
        );

        let mut extractor = Extractor::default();
        extractor.visit_program(&program);
        for symbol in &mut extractor.symbols {
            set_range_lines(source, &mut symbol.range);
        }
        for call in &mut extractor.calls {
            set_range_lines(source, &mut call.range);
        }
        let resolver = self.root.as_ref().map(|_| {
            let options = ResolveOptions {
                extensions: vec![
                    ".ts".to_owned(),
                    ".tsx".to_owned(),
                    ".js".to_owned(),
                    ".jsx".to_owned(),
                    ".mts".to_owned(),
                    ".cts".to_owned(),
                    ".mjs".to_owned(),
                    ".cjs".to_owned(),
                    ".json".to_owned(),
                    "".to_owned(),
                ],
                tsconfig: Some(TsconfigDiscovery::Auto),
                ..ResolveOptions::default()
            };
            Resolver::new(options)
        });
        if let Some(resolver) = &resolver {
            let absolute_path = self
                .root
                .as_ref()
                .map(|root| root.join(path))
                .unwrap_or_else(|| path.to_path_buf());
            for import in &mut extractor.imports {
                import.resolved_path = resolver
                    .resolve_file(&absolute_path, &import.source)
                    .ok()
                    .map(|resolution| resolution.path().to_path_buf())
                    .and_then(|resolved| {
                        self.root.as_ref().and_then(|root| {
                            let resolved = resolved.canonicalize().unwrap_or(resolved);
                            let root = root.canonicalize().unwrap_or_else(|_| root.clone());
                            resolved
                                .strip_prefix(&root)
                                .or_else(|_| resolved.strip_prefix(root))
                                .ok()
                                .map(std::path::Path::to_path_buf)
                        })
                    });
            }
        }

        let scoping = semantic.semantic.scoping();
        for (symbol, symbol_id) in extractor.symbols.iter_mut().zip(&extractor.symbol_ids) {
            if let Some(symbol_id) = symbol_id {
                let scope_id = scoping.symbol_scope_id(*symbol_id);
                symbol.qualified_name = Some(format!("{}@{}", symbol.name, scope_id.index()));
            }
        }
        let references = extractor
            .identifiers
            .into_iter()
            .map(|identifier| {
                let (target, kind) = identifier
                    .reference_id
                    .map(|reference_id| {
                        let reference = scoping.get_reference(reference_id);
                        let target = scoping.get_reference_name(reference_id).map(str::to_owned);
                        let kind = if extractor.call_ranges.iter().any(|range| {
                            range.0 <= identifier.span.start && identifier.span.end <= range.1
                        }) {
                            ReferenceKind::Call
                        } else if reference.is_write() && !reference.is_read() {
                            ReferenceKind::Write
                        } else if reference.is_read() {
                            ReferenceKind::Read
                        } else {
                            ReferenceKind::Unknown
                        };
                        (target, kind)
                    })
                    .unwrap_or((None, ReferenceKind::Unknown));
                Reference {
                    name: identifier.name,
                    target,
                    kind,
                    range: source_range(source, identifier.span),
                }
            })
            .collect();

        Ok(AnalysisResult {
            path: path.to_path_buf(),
            analyzer: AnalyzerMetadata {
                name: "oxc".to_owned(),
                version: "0.140.0".to_owned(),
            },
            symbols: extractor.symbols,
            references,
            imports: extractor.imports,
            exports: extractor.exports,
            calls: extractor.calls,
            diagnostics,
        })
    }
}

fn empty_result(path: &Path, diagnostics: Vec<Diagnostic>) -> AnalysisResult {
    AnalysisResult {
        path: path.to_path_buf(),
        analyzer: AnalyzerMetadata {
            name: "oxc".to_owned(),
            version: "0.140.0".to_owned(),
        },
        symbols: Vec::new(),
        references: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        calls: Vec::new(),
        diagnostics,
    }
}

fn diagnostic_to_model(message: String) -> Diagnostic {
    Diagnostic {
        message,
        severity: DiagnosticSeverity::Error,
        range: None,
    }
}

fn source_range(source: &str, span: Span) -> SourceRange {
    let start = span.start as usize;
    let end = span.end as usize;
    SourceRange {
        start_byte: start,
        end_byte: end,
        start_line: source[..start.min(source.len())]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
        end_line: source[..end.min(source.len())]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
    }
}

fn set_range_lines(source: &str, range: &mut SourceRange) {
    range.start_line = line_at(source, range.start_byte);
    range.end_line = line_at(source, range.end_byte);
}

fn line_at(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())]
        .bytes()
        .filter(|value| *value == b'\n')
        .count()
        + 1
}

#[derive(Debug, Clone)]
struct IdentifierReferenceRecord {
    name: String,
    span: Span,
    reference_id: Option<oxc_semantic::ReferenceId>,
}

#[derive(Debug, Default)]
struct Extractor {
    symbols: Vec<Symbol>,
    symbol_ids: Vec<Option<oxc_semantic::SymbolId>>,
    imports: Vec<Import>,
    exports: Vec<Export>,
    calls: Vec<Call>,
    identifiers: Vec<IdentifierReferenceRecord>,
    call_ranges: Vec<(u32, u32)>,
}

impl<'a> Visit<'a> for Extractor {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.identifiers.push(IdentifierReferenceRecord {
            name: it.name.to_string(),
            span: it.span,
            reference_id: it.reference_id.get(),
        });
        walk::walk_identifier_reference(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(identifier) = &it.id {
            let kind = it.init.as_ref().map_or(SymbolKind::Variable, |init| {
                matches!(
                    init,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                )
                .then_some(SymbolKind::Function)
                .unwrap_or(SymbolKind::Variable)
            });
            self.add_symbol(
                identifier.name.to_string(),
                kind,
                it.span,
                identifier.symbol_id.get(),
            );
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_semantic::ScopeFlags) {
        if let Some(identifier) = &it.id {
            self.add_symbol(
                identifier.name.to_string(),
                SymbolKind::Function,
                it.span,
                identifier.symbol_id.get(),
            );
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        if let Some(identifier) = &it.id {
            self.add_symbol(
                identifier.name.to_string(),
                SymbolKind::Class,
                it.span,
                identifier.symbol_id.get(),
            );
        }
        walk::walk_class(self, it);
    }

    fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'a>) {
        self.add_symbol(
            it.id.name.to_string(),
            SymbolKind::Type,
            it.span,
            it.id.symbol_id.get(),
        );
        walk::walk_ts_type_alias_declaration(self, it);
    }

    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'a>) {
        self.add_symbol(
            it.id.name.to_string(),
            SymbolKind::Interface,
            it.span,
            it.id.symbol_id.get(),
        );
        walk::walk_ts_interface_declaration(self, it);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        let source = it.source.value.to_string();
        if let Some(specifiers) = &it.specifiers {
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        self.imports.push(Import {
                            source: source.clone(),
                            imported_name: Some(module_export_name(&specifier.imported)),
                            local_name: Some(specifier.local.name.to_string()),
                            resolved_path: None,
                        });
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        self.imports.push(Import {
                            source: source.clone(),
                            imported_name: Some("default".to_owned()),
                            local_name: Some(specifier.local.name.to_string()),
                            resolved_path: None,
                        });
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        self.imports.push(Import {
                            source: source.clone(),
                            imported_name: Some("*".to_owned()),
                            local_name: Some(specifier.local.name.to_string()),
                            resolved_path: None,
                        });
                    }
                }
            }
        } else {
            self.imports.push(Import {
                source,
                imported_name: None,
                local_name: None,
                resolved_path: None,
            });
        }
        walk::walk_import_declaration(self, it);
    }

    fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
        if let Some(declaration) = &it.declaration {
            if let Some((name, local_name)) = declaration_name(declaration) {
                self.exports.push(Export {
                    exported_name: name,
                    local_name,
                });
            }
        }
        for specifier in &it.specifiers {
            self.exports.push(Export {
                exported_name: module_export_name(&specifier.exported),
                local_name: Some(module_export_name(&specifier.local)),
            });
        }
        walk::walk_export_named_declaration(self, it);
    }

    fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'a>) {
        let local_name = match &it.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                function.id.as_ref().map(|id| id.name.to_string())
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                class.id.as_ref().map(|id| id.name.to_string())
            }
            _ => None,
        };
        self.exports.push(Export {
            exported_name: "default".to_owned(),
            local_name,
        });
        walk::walk_export_default_declaration(self, it);
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.exports.push(Export {
            exported_name: it
                .exported
                .as_ref()
                .map_or_else(|| "*".to_owned(), module_export_name),
            local_name: None,
        });
        walk::walk_export_all_declaration(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let callee = expression_name(&it.callee).unwrap_or_else(|| "<expression>".to_owned());
        self.calls.push(Call {
            callee,
            range: SourceRange {
                start_byte: it.span.start as usize,
                end_byte: it.span.end as usize,
                start_line: 0,
                end_line: 0,
            },
        });
        self.call_ranges.push((it.span.start, it.span.end));
        walk::walk_call_expression(self, it);
    }
}

impl Extractor {
    fn add_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        span: Span,
        symbol_id: Option<oxc_semantic::SymbolId>,
    ) {
        self.symbols.push(Symbol {
            name,
            qualified_name: None,
            kind,
            range: SourceRange {
                start_byte: span.start as usize,
                end_byte: span.end as usize,
                start_line: 0,
                end_line: 0,
            },
        });
        self.symbol_ids.push(symbol_id);
    }
}

fn declaration_name(declaration: &Declaration<'_>) -> Option<(String, Option<String>)> {
    match declaration {
        Declaration::VariableDeclaration(_) => None,
        Declaration::FunctionDeclaration(function) => function
            .id
            .as_ref()
            .map(|id| (id.name.to_string(), Some(id.name.to_string()))),
        Declaration::ClassDeclaration(class) => class
            .id
            .as_ref()
            .map(|id| (id.name.to_string(), Some(id.name.to_string()))),
        Declaration::TSTypeAliasDeclaration(alias) => {
            Some((alias.id.name.to_string(), Some(alias.id.name.to_string())))
        }
        Declaration::TSInterfaceDeclaration(interface) => Some((
            interface.id.name.to_string(),
            Some(interface.id.name.to_string()),
        )),
        Declaration::TSEnumDeclaration(enum_declaration) => Some((
            enum_declaration.id.name.to_string(),
            Some(enum_declaration.id.name.to_string()),
        )),
        Declaration::TSModuleDeclaration(module) => Some((module.id.name().to_string(), None)),
        Declaration::TSGlobalDeclaration(_) | Declaration::TSImportEqualsDeclaration(_) => None,
    }
}

fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

fn expression_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => expression_name(&member.object)
            .map(|object| format!("{object}.{}", member.property.name)),
        _ => None,
    }
}
