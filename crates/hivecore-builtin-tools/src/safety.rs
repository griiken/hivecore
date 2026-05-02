//! Path safety. Every builtin tool must resolve user-supplied paths through
//! `WorkspaceRoot::resolve` before touching the filesystem. The resolved
//! path is guaranteed to live under the canonicalised workspace root.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::error::ToolError;

#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    inner: Arc<PathBuf>,
}

impl WorkspaceRoot {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, ToolError> {
        let path = path
            .into()
            .canonicalize()
            .map_err(|e| ToolError::InvalidArg(format!("workspace root not accessible: {e}")))?;
        Ok(Self {
            inner: Arc::new(path),
        })
    }

    pub fn path(&self) -> &Path {
        self.inner.as_path()
    }

    /// Resolve a user-supplied path against the root.
    ///
    /// - Absolute paths must already live under the root.
    /// - Relative paths join the root.
    /// - `..` is rejected before canonicalisation (avoids TOCTOU via symlink
    ///   redirects).
    pub fn resolve(&self, candidate: &str) -> Result<PathBuf, ToolError> {
        let candidate = Path::new(candidate);
        if candidate
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(ToolError::PathEscape(candidate.display().to_string()));
        }
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.inner.join(candidate)
        };
        // Canonicalise the longest existing prefix and verify containment.
        let real = longest_existing_prefix(&joined);
        let canon = real.canonicalize().map_err(ToolError::Io)?;
        if !canon.starts_with(self.inner.as_path()) {
            return Err(ToolError::PathEscape(joined.display().to_string()));
        }
        // Re-attach the non-existing tail (for create/write paths).
        let tail = joined.strip_prefix(&real).unwrap_or_else(|_| Path::new(""));
        if tail.as_os_str().is_empty() {
            Ok(canon)
        } else {
            Ok(canon.join(tail))
        }
    }
}

fn longest_existing_prefix(p: &Path) -> PathBuf {
    let mut current = p.to_path_buf();
    while !current.exists() {
        if !current.pop() {
            break;
        }
    }
    current
}

#[cfg(test)]
#[path = "safety_tests.rs"]
mod tests;
