use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use rusqlite::{params, Connection, OptionalExtension};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredRepository {
    pub name: String,
    pub root: RepositoryRoot,
}

#[derive(Debug, Clone)]
pub struct RepositoryRegistry {
    database_path: PathBuf,
}

impl RepositoryRegistry {
    pub fn new() -> Self {
        Self::at(default_registry_path())
    }

    pub fn at(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }

    pub fn default_path() -> PathBuf {
        default_registry_path()
    }

    pub fn register(
        &self,
        name: &str,
        root: impl AsRef<Path>,
        replace: bool,
    ) -> Result<RegisteredRepository> {
        validate_repository_name(name)?;
        let root = RepositoryRoot::resolve(root)?;
        let root_path = root.path().to_string_lossy().into_owned();
        let connection = self.open_connection()?;
        let now = now_string();
        let existing = connection
            .query_row(
                "SELECT root_path FROM repositories WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?;

        if let Some(existing_root) = existing {
            if existing_root != root_path && !replace {
                return Err(AstralError::RepositoryNameConflict {
                    name: name.to_owned(),
                });
            }
            if existing_root == root_path {
                connection
                    .execute(
                        "UPDATE repositories SET updated_at = ?2 WHERE name = ?1",
                        params![name, now],
                    )
                    .map_err(database_error)?;
            } else {
                let other_name = connection
                    .query_row(
                        "SELECT name FROM repositories WHERE root_path = ?1 AND name <> ?2",
                        params![root_path, name],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(database_error)?;
                if other_name.is_some() {
                    return Err(AstralError::RepositoryRootConflict {
                        path: root.path().to_path_buf(),
                    });
                }
                connection
                    .execute(
                        "UPDATE repositories SET root_path = ?2, updated_at = ?3 WHERE name = ?1",
                        params![name, root_path, now],
                    )
                    .map_err(database_error)?;
            }
        } else {
            let other_name = connection
                .query_row(
                    "SELECT name FROM repositories WHERE root_path = ?1",
                    [&root_path],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(database_error)?;
            if other_name.is_some() {
                return Err(AstralError::RepositoryRootConflict {
                    path: root.path().to_path_buf(),
                });
            }
            connection
                .execute(
                    "INSERT INTO repositories(name, root_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                    params![name, root_path, now],
                )
                .map_err(database_error)?;
        }

        Ok(RegisteredRepository {
            name: name.to_owned(),
            root,
        })
    }

    pub fn resolve(&self, name: &str) -> Result<RegisteredRepository> {
        validate_repository_name(name)?;
        let connection = self.open_connection()?;
        let root_path = connection
            .query_row(
                "SELECT root_path FROM repositories WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AstralError::RepositoryNotRegistered {
                name: name.to_owned(),
            })?;
        let registered_path = PathBuf::from(root_path);
        let root = RepositoryRoot::resolve(&registered_path)?;
        if root.path() != registered_path {
            return Err(AstralError::RepositoryRootNotFound {
                path: registered_path,
            });
        }
        Ok(RegisteredRepository {
            name: name.to_owned(),
            root,
        })
    }

    pub fn registered_root_path(&self, name: &str) -> Result<PathBuf> {
        validate_repository_name(name)?;
        let connection = self.open_connection()?;
        connection
            .query_row(
                "SELECT root_path FROM repositories WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.map(PathBuf::from))
            .map_err(database_error)?
            .ok_or_else(|| AstralError::RepositoryNotRegistered {
                name: name.to_owned(),
            })
    }

    pub fn unregister(&self, name: &str) -> Result<()> {
        validate_repository_name(name)?;
        let connection = self.open_connection()?;
        let deleted = connection
            .execute("DELETE FROM repositories WHERE name = ?1", [name])
            .map_err(database_error)?;
        if deleted == 0 {
            return Err(AstralError::RepositoryNotRegistered {
                name: name.to_owned(),
            });
        }
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection> {
        if let Some(parent) = self.database_path.parent() {
            fs::create_dir_all(parent).map_err(|source| AstralError::PathAccess {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&self.database_path).map_err(database_error)?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS repositories(
                    name TEXT PRIMARY KEY,
                    root_path TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
            )
            .map_err(database_error)?;
        Ok(connection)
    }
}

impl Default for RepositoryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_data_dir() -> PathBuf {
    std::env::var_os("ASTRAL_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            ProjectDirs::from("com", "astral", "astral")
                .map(|directories| directories.data_dir().to_path_buf())
        })
        .unwrap_or_else(|| PathBuf::from(".astral"))
}

fn default_registry_path() -> PathBuf {
    default_data_dir().join("registry.sqlite")
}

pub(crate) fn validate_repository_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AstralError::InvalidRepositoryName {
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn database_error(error: rusqlite::Error) -> AstralError {
    AstralError::Database {
        message: error.to_string(),
    }
}

fn has_git_metadata(path: &Path) -> bool {
    fs::symlink_metadata(path.join(".git")).is_ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{RegisteredRepository, RepositoryRegistry, RepositoryRoot};
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

    #[test]
    fn registers_and_resolves_a_repository_by_name() {
        let directory = tempdir().expect("temporary directory");
        fs::create_dir(directory.path().join(".git")).expect("git metadata");
        let registry = RepositoryRegistry::at(directory.path().join("registry.sqlite"));

        let registered = registry
            .register("astral", directory.path(), false)
            .expect("register repository");
        let resolved = registry.resolve("astral").expect("resolve repository");

        assert_eq!(registered, resolved);
        assert_eq!(
            resolved,
            RegisteredRepository {
                name: "astral".to_owned(),
                root: RepositoryRoot(directory.path().canonicalize().unwrap()),
            }
        );
    }

    #[test]
    fn rejects_invalid_and_unknown_repository_names() {
        let directory = tempdir().expect("temporary directory");
        let registry = RepositoryRegistry::at(directory.path().join("registry.sqlite"));

        assert!(registry
            .register("bad/name", directory.path(), false)
            .is_err());
        assert!(registry.resolve("missing").is_err());
    }

    #[test]
    fn rejects_conflicting_registration_without_replace() {
        let registry_dir = tempdir().expect("registry directory");
        let first = tempdir().expect("first repository");
        let second = tempdir().expect("second repository");
        fs::create_dir(first.path().join(".git")).expect("first git metadata");
        fs::create_dir(second.path().join(".git")).expect("second git metadata");
        let registry = RepositoryRegistry::at(registry_dir.path().join("registry.sqlite"));

        registry
            .register("astral", first.path(), false)
            .expect("initial registration");
        assert!(registry.register("astral", second.path(), false).is_err());
        assert!(registry.register("other", first.path(), false).is_err());
        registry
            .register("astral", second.path(), true)
            .expect("replacement registration");
    }

    #[test]
    fn unregisters_a_repository_name() {
        let directory = tempdir().expect("temporary repository");
        fs::create_dir(directory.path().join(".git")).expect("git metadata");
        let registry = RepositoryRegistry::at(directory.path().join("registry.sqlite"));
        registry
            .register("astral", directory.path(), false)
            .expect("register repository");

        registry
            .unregister("astral")
            .expect("unregister repository");

        assert!(registry.resolve("astral").is_err());
    }
}
