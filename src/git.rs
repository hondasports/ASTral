use std::{path::Path, process::Command};

use sha2::{Digest, Sha256};

use crate::error::{AstralError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    pub available: bool,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub dirty: bool,
    pub dirty_file_count: usize,
    pub worktree_hash: Option<String>,
}

pub fn inspect(root: &Path) -> Result<GitSnapshot> {
    let head = match git_output(root, &["rev-parse", "HEAD"])? {
        Some(value) => value,
        None => {
            return Ok(GitSnapshot {
                available: false,
                head: None,
                branch: None,
                dirty: false,
                dirty_file_count: 0,
                worktree_hash: None,
            });
        }
    };
    let branch = git_output(root, &["symbolic-ref", "--short", "-q", "HEAD"])?;
    let status = git_bytes(root, &["status", "--porcelain=v1", "-z"])?.unwrap_or_default();
    let dirty_file_count = status
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .count();
    let dirty = dirty_file_count > 0;
    let mut hasher = Sha256::new();
    hasher.update(&status);
    let worktree_hash = Some(format!("{:x}", hasher.finalize()));
    Ok(GitSnapshot {
        available: true,
        head: Some(head),
        branch,
        dirty,
        dirty_file_count,
        worktree_hash,
    })
}

fn git_output(root: &Path, args: &[&str]) -> Result<Option<String>> {
    Ok(git_bytes(root, args)?.and_then(|bytes| {
        let value = String::from_utf8_lossy(&bytes).trim().to_owned();
        (!value.is_empty()).then_some(value)
    }))
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| AstralError::PathAccess {
            path: root.to_path_buf(),
            source,
        });
    let output = match output {
        Ok(output) => output,
        Err(error) if error.to_string().contains("not recognized") => return Ok(None),
        Err(error) => return Err(error),
    };
    if output.status.success() {
        Ok(Some(output.stdout))
    } else if args.first() == Some(&"rev-parse") {
        Ok(None)
    } else {
        Err(AstralError::Indexing {
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}
