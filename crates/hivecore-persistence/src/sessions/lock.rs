//! Advisory lock file for a session — prevents concurrent writers.
//!
//! Pattern: write `<session_path>.lock` containing this process's PID on
//! `acquire()`; remove on `Drop`. If the lock file already exists and the
//! recorded PID is alive, `acquire()` fails with `Busy`. Stale locks
//! (PID gone) are reclaimed.
//!
//! No prior art for this in the OSS coding-agent space — added defensively
//! per ADR-026 because hivecore embeds into multi-process scenarios
//! (CLI + ACP server + scheduled agents). Codex/pi/jcode don't lock.

use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Error)]
pub enum LockError {
    #[error("session in use by pid {pid} (lock file: {path:?}). Use --fork to branch.")]
    Busy { pid: u32, path: PathBuf },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// RAII guard. Drop releases the lock file (best-effort, may leak on hard
/// kill — acquire() will reclaim stale locks on next attempt).
#[derive(Debug)]
pub struct SessionLock {
    path: PathBuf,
}

impl SessionLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Acquire `<session_path>.lock`. Reclaims stale locks (PID no longer
/// alive). Returns `Busy` if a live process holds the lock.
pub async fn acquire(session_path: &Path) -> Result<SessionLock, LockError> {
    let lock_path = lock_path(session_path);

    // Ensure parent directory exists — caller may invoke this before the
    // session file itself is created.
    if let Some(parent) = lock_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // If lock exists, check liveness.
    if lock_path.exists() {
        let contents = tokio::fs::read_to_string(&lock_path)
            .await
            .unwrap_or_default();
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid_alive(pid) {
                return Err(LockError::Busy {
                    pid,
                    path: lock_path,
                });
            }
            tracing::warn!(pid, path = %lock_path.display(), "reclaiming stale session lock");
            tokio::fs::remove_file(&lock_path).await?;
        } else {
            // Garbage contents — reclaim.
            tokio::fs::remove_file(&lock_path).await?;
        }
    }

    // Create exclusive — fails if a racing process beat us.
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .await?;
    f.write_all(format!("{}", std::process::id()).as_bytes())
        .await?;
    f.flush().await?;
    drop(f);

    Ok(SessionLock { path: lock_path })
}

fn lock_path(session_path: &Path) -> PathBuf {
    let mut p = session_path.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // `kill -0` semantics — sends signal 0, returns Ok if process exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // Conservative: assume alive on non-unix. Forces user to clean up.
    true
}
