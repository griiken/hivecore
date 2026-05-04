//! Pluggable I/O for builtin tools (ADR-031).
//!
//! `ExecutionEnv` is the trait that every fs+process call from a tool routes
//! through. The default impl `LocalEnv` lives in the sibling crate
//! `hivecore-execution-env` (kept out of `runtime-core` to preserve the
//! zero-I/O rule from CLAUDE.md / AGENTS.md). Future sandbox providers —
//! Firecracker microVMs, Docker containers, remote SSH boxes, leased cloud
//! runners — drop in as additional impls without touching tool code.
//!
//! `WorkspaceRoot` (in `hivecore-builtin-tools::safety`) continues to own
//! path validation; `ExecutionEnv` owns the *execution* of the resolved
//! action. Defence-in-depth — they layer.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::abort::AbortSignal;
use crate::error::RuntimeResult;

/// Options controlling a single `exec()` invocation.
#[derive(Debug, Default, Clone)]
pub struct ExecOpts {
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub timeout: Option<Duration>,
    pub signal: Option<AbortSignal>,
}

/// Result of a process invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl ExecOutput {
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

/// File metadata.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mtime: SystemTime,
}

/// Options controlling a `remove()` call.
#[derive(Debug, Default, Clone, Copy)]
pub struct RemoveOpts {
    pub recursive: bool,
    pub force: bool,
}

/// Pluggable filesystem + process I/O for tools.
///
/// Implementations cover the local OS (`LocalEnv`), in-memory test fixtures
/// (`MemoryEnv`, future), and remote sandbox providers. Each impl is fully
/// self-contained — a `(tenant, session)` pair can be assigned a freshly-leased
/// env and tools pick it up without code changes.
#[async_trait]
pub trait ExecutionEnv: Send + Sync + Debug {
    /// Working directory the env was constructed with. Path-resolution and
    /// `cwd`-defaulted operations route through this.
    fn cwd(&self) -> &Path;

    /// Spawn a process. Default shell semantics are impl-defined; `LocalEnv`
    /// uses `bash -lc <command>` on Unix.
    async fn exec(&self, command: &str, opts: ExecOpts) -> RuntimeResult<ExecOutput>;

    async fn read_file(&self, path: &Path) -> RuntimeResult<Vec<u8>>;
    async fn read_text_file(&self, path: &Path) -> RuntimeResult<String>;
    async fn write_file(&self, path: &Path, bytes: &[u8]) -> RuntimeResult<()>;
    async fn stat(&self, path: &Path) -> RuntimeResult<FileStat>;
    async fn list_dir(&self, path: &Path) -> RuntimeResult<Vec<PathBuf>>;
    async fn path_exists(&self, path: &Path) -> RuntimeResult<bool>;
    async fn create_dir(&self, path: &Path, recursive: bool) -> RuntimeResult<()>;
    async fn remove(&self, path: &Path, opts: RemoveOpts) -> RuntimeResult<()>;

    async fn create_temp_dir(&self, prefix: Option<&str>) -> RuntimeResult<PathBuf>;
    async fn create_temp_file(
        &self,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> RuntimeResult<PathBuf>;

    /// Resolve a possibly-relative path under the env's `cwd`. Pure;
    /// implementations should not perform I/O here.
    fn resolve_path(&self, path: &Path) -> PathBuf;

    /// Best-effort cleanup on shutdown. Idempotent.
    async fn cleanup(&self) -> RuntimeResult<()>;
}

#[cfg(test)]
#[path = "execution_env_tests.rs"]
mod tests;
