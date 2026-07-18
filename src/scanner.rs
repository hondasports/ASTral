use std::{
    fs,
    path::{Path, PathBuf},
};

use ignore::WalkBuilder;

use crate::{AstralError, Result};

const SUPPORTED_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub absolute_path: PathBuf,
    pub relative_path: PathBuf,
    pub language: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct SourceScanner {
    root: PathBuf,
}

impl SourceScanner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn scan(&self) -> Result<Vec<SourceFile>> {
        let root = self
            .root
            .canonicalize()
            .map_err(|source| AstralError::Canonicalize {
                path: self.root.clone(),
                source,
            })?;
        if !root.is_dir() {
            return Err(AstralError::NotDirectory { path: root });
        }

        let walker = WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(false)
            .parents(false)
            .filter_entry(|entry| !is_excluded_directory(entry.path()))
            .build();

        let mut files = Vec::new();
        for entry in walker {
            let entry = entry.map_err(|error| AstralError::PathAccess {
                path: root.clone(),
                source: std::io::Error::other(error.to_string()),
            })?;
            let path = entry.path();
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
                || is_secret_or_generated(path)
            {
                continue;
            }
            let Some(language) = language_for(path) else {
                continue;
            };
            let source = fs::read_to_string(path).map_err(|source| AstralError::PathAccess {
                path: path.to_path_buf(),
                source,
            })?;
            let relative_path = path
                .strip_prefix(&root)
                .map(Path::to_path_buf)
                .map_err(|_| AstralError::PathAccess {
                    path: path.to_path_buf(),
                    source: std::io::Error::other("path is outside scanner root"),
                })?;
            files.push(SourceFile {
                absolute_path: path.to_path_buf(),
                relative_path,
                language: language.to_owned(),
                source,
            });
        }

        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(files)
    }
}

pub fn language_for(path: &Path) -> Option<&'static str> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .find(|supported| supported.eq_ignore_ascii_case(extension))
                .copied()
        })
}

fn is_excluded_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | "coverage"
            )
        })
}

fn is_secret_or_generated(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.starts_with(".env")
        || name.ends_with(".map")
        || name.ends_with(".min.js")
        || name.ends_with(".min.ts")
        || name.ends_with(".min.tsx")
}

#[cfg(test)]
mod tests {
    use super::language_for;
    use std::path::Path;

    #[test]
    fn recognizes_javascript_and_typescript_extensions() {
        assert_eq!(language_for(Path::new("file.JSX")), Some("jsx"));
        assert_eq!(language_for(Path::new("file.tsx")), Some("tsx"));
        assert_eq!(language_for(Path::new("README.md")), None);
    }
}
