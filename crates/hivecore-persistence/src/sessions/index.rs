//! Append-only session index — Codex pattern (`session_index.jsonl`).
//!
//! Maps `(session_id ↔ name ↔ cwd ↔ updated_at)`. Every rename or session
//! creation appends a new line; **the most recent line wins** on lookup.
//!
//! Lookups scan from end-of-file backwards. Cheap: we never read the whole
//! file when we hit a match. Index lives at
//! `<sessions_dir>/<tenant_id>/_index.jsonl`.
//!
//! Mirrors `openai/codex/codex-rs/rollout/src/session_index.rs` semantics.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use hivecore_runtime_core::SessionId;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use crate::error::Result;

/// One append-only line in `_index.jsonl`. Multiple entries for the same
/// `session_id` are normal (rename, cwd change, touch on reopen) — the
/// last-written entry takes precedence on lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    pub session_id: SessionId,
    pub cwd: PathBuf,
    /// User-assigned handle (e.g. "auth-refactor"). Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

const INDEX_FILE: &str = "_index.jsonl";

/// Path to the index for a given tenant directory
/// (`<sessions_dir>/<tenant_id>/_index.jsonl`).
pub fn index_path(tenant_dir: &Path) -> PathBuf {
    tenant_dir.join(INDEX_FILE)
}

/// Append a new entry to the index. Creates parent dir + file if missing.
pub async fn append_entry(tenant_dir: &Path, entry: &SessionIndexEntry) -> Result<()> {
    tokio::fs::create_dir_all(tenant_dir).await?;
    let path = index_path(tenant_dir);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    file.write_all(line.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Most recent entry for the given `session_id`, if any. Forward scan; the
/// last match wins (append-order = chronological).
pub async fn find_by_id(tenant_dir: &Path, id: SessionId) -> Result<Option<SessionIndexEntry>> {
    let path = index_path(tenant_dir);
    if !path.exists() {
        return Ok(None);
    }
    let mut last: Option<SessionIndexEntry> = None;
    let mut lines = tokio::io::BufReader::new(tokio::fs::File::open(&path).await?).lines();
    while let Some(line) = lines.next_line().await? {
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(&line) else {
            continue;
        };
        if entry.session_id == id {
            last = Some(entry);
        }
    }
    Ok(last)
}

/// Most recent entry whose `name` matches, if any. Last-match wins.
pub async fn find_by_name(tenant_dir: &Path, name: &str) -> Result<Option<SessionIndexEntry>> {
    let path = index_path(tenant_dir);
    if !path.exists() {
        return Ok(None);
    }
    let mut last: Option<SessionIndexEntry> = None;
    let mut lines = tokio::io::BufReader::new(tokio::fs::File::open(&path).await?).lines();
    while let Some(line) = lines.next_line().await? {
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(&line) else {
            continue;
        };
        if entry.name.as_deref() == Some(name) {
            last = Some(entry);
        }
    }
    Ok(last)
}

/// Most recently-updated session whose `cwd` matches `cwd` exactly.
/// Drives `--continue` (latest-by-cwd UX).
pub async fn find_latest_by_cwd(
    tenant_dir: &Path,
    cwd: &Path,
) -> Result<Option<SessionIndexEntry>> {
    let path = index_path(tenant_dir);
    if !path.exists() {
        return Ok(None);
    }
    let mut latest: Option<SessionIndexEntry> = None;
    let mut lines = tokio::io::BufReader::new(tokio::fs::File::open(&path).await?).lines();
    while let Some(line) = lines.next_line().await? {
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(&line) else {
            continue;
        };
        if entry.cwd == cwd {
            // Append-order = chronological; last match for this cwd wins.
            latest = Some(entry);
        }
    }
    Ok(latest)
}

/// Resolve a user-supplied handle to a session id. Tries name first, then
/// parses as a UUID. Returns `None` if neither matches.
pub async fn resolve_handle(tenant_dir: &Path, handle: &str) -> Result<Option<SessionIndexEntry>> {
    if let Some(by_name) = find_by_name(tenant_dir, handle).await? {
        return Ok(Some(by_name));
    }
    if let Ok(uuid) = uuid::Uuid::parse_str(handle) {
        return find_by_id(tenant_dir, SessionId(uuid)).await;
    }
    Ok(None)
}
