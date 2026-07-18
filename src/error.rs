use std::{io, path::PathBuf};

pub type Result<T> = std::result::Result<T, AstralError>;

#[derive(Debug, thiserror::Error)]
pub enum AstralError {
    #[error("path does not exist: {path}")]
    PathNotFound { path: PathBuf },

    #[error("path is not a directory: {path}")]
    NotDirectory { path: PathBuf },

    #[error("repository root not found from path: {path}")]
    RepositoryRootNotFound { path: PathBuf },

    #[error("failed to access path '{path}': {source}")]
    PathAccess {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to canonicalize path '{path}': {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid configuration: {message}")]
    InvalidConfiguration { message: String },

    #[error("failed to initialize logging: {message}")]
    Logging { message: String },

    #[error("database operation failed: {message}")]
    Database { message: String },

    #[error("indexing failed: {message}")]
    Indexing { message: String },
}
