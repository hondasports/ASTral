use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::{AstralError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRoot(PathBuf);

impl RepositoryRoot {
    pub fn resolve(input: impl AsRef<Path>) -> Result<Self> {
        let input = input.as_ref();
        let metadata = fs::metadata(input).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                AstralError::PathNotFound {
                    path: input.to_path_buf(),
                }
            } else {
                AstralError::PathAccess {
                    path: input.to_path_buf(),
                    source,
                }
            }
        })?;

        if !metadata.is_dir() {
            return Err(AstralError::NotDirectory {
                path: input.to_path_buf(),
            });
        }

        let canonical = fs::canonicalize(input).map_err(|source| AstralError::Canonicalize {
            path: input.to_path_buf(),
            source,
        })?;

        let mut candidate = canonical.as_path();
        loop {
            if has_git_metadata(candidate) {
                return Ok(Self(candidate.to_path_buf()));
            }

            candidate = match candidate.parent() {
                Some(parent) => parent,
                None => break,
            };
        }

        Err(AstralError::RepositoryRootNotFound { path: canonical })
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

fn has_git_metadata(path: &Path) -> bool {
    fs::symlink_metadata(path.join(".git")).is_ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::RepositoryRoot;
    use crate::AstralError;

    #[test]
    fn resolves_the_nearest_repository_root_from_a_child() {
        let directory = tempdir().expect("temporary directory");
        fs::create_dir(directory.path().join(".git")).expect("git metadata");
        let child = directory.path().join("src").join("feature");
        fs::create_dir_all(&child).expect("child directory");

        let root = RepositoryRoot::resolve(&child).expect("repository root");

        assert_eq!(root.path(), directory.path().canonicalize().unwrap());
    }

    #[test]
    fn accepts_a_git_file_used_by_worktrees() {
        let directory = tempdir().expect("temporary directory");
        fs::write(directory.path().join(".git"), "gitdir: /tmp/worktree").expect("git file");

        let root = RepositoryRoot::resolve(directory.path()).expect("repository root");

        assert_eq!(root.path(), directory.path().canonicalize().unwrap());
    }

    #[test]
    fn rejects_a_missing_path() {
        let directory = tempdir().expect("temporary directory");
        let missing = directory.path().join("missing");

        assert!(matches!(
            RepositoryRoot::resolve(&missing),
            Err(AstralError::PathNotFound { .. })
        ));
    }

    #[test]
    fn rejects_a_file_path() {
        let directory = tempdir().expect("temporary directory");
        let file = directory.path().join("file");
        fs::write(&file, "content").expect("file");

        assert!(matches!(
            RepositoryRoot::resolve(&file),
            Err(AstralError::NotDirectory { .. })
        ));
    }

    #[test]
    fn rejects_a_path_without_git_metadata() {
        let directory = tempdir().expect("temporary directory");

        assert!(matches!(
            RepositoryRoot::resolve(directory.path()),
            Err(AstralError::RepositoryRootNotFound { .. })
        ));
    }
}
