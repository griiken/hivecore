//! Session JSONL — one file per `SessionId`. Append-only, tree-shaped
//! (ADR-032; supersedes ADR-026's linear schema). Each non-header line is
//! a `SessionEntry` carrying `id` + `parent_id` so navigation, fork,
//! rewind, and per-branch compaction are first-class operations on the
//! same on-disk artefact.
//!
//! 032a (this commit) introduces the schema; the agent loop continues to
//! operate linearly — every `append_message` extends the current leaf and
//! a `LeafChange` entry journals the cursor move. 032b adds the
//! `SessionStore` trait, branch navigation API, and `--fork-from` CLI.
//!
//! Format `version: 2` is incompatible with `version: 1` files; pre-launch
//! we have no users on disk so this is a clean break (per ADR-032).

pub mod index;
pub mod lock;
mod reader;
pub mod store;
mod writer;

pub use index::{
    append_entry, find_by_id, find_by_name, find_latest_by_cwd, index_path, resolve_handle,
    SessionIndexEntry,
};
pub use lock::{acquire as acquire_lock, LockError, SessionLock};
pub use reader::SessionReader;
pub use store::{SessionEntryKind, SessionMetadata, SessionStore};
pub use writer::SessionWriter;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use hivecore_runtime_core::{AgentEvent, AgentMessage, SessionId};
use serde::{Deserialize, Serialize};

/// Build the canonical session JSONL path:
/// `<sessions_dir>/<tenant_id>/<session_id>.jsonl` (ADR-026 invariant
/// preserved by ADR-032).
pub fn session_path(sessions_dir: &Path, tenant_id: &str, session_id: SessionId) -> PathBuf {
    sessions_dir
        .join(tenant_id)
        .join(format!("{}.jsonl", session_id.0))
}

/// Build the per-tenant directory: `<sessions_dir>/<tenant_id>/`.
pub fn tenant_dir(sessions_dir: &Path, tenant_id: &str) -> PathBuf {
    sessions_dir.join(tenant_id)
}

/// 8-char prefix of a UUIDv4 (collision-checked at allocation in
/// `SessionWriter::next_entry_id`). Pi-mono uses an 8-char id; we match
/// for human-readability of the on-disk log.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryId(pub String);

impl EntryId {
    pub fn new() -> Self {
        // Take the first 8 hex chars of a fresh v4 UUID. Caller (writer)
        // must guard against collisions inside the open file.
        let s = uuid::Uuid::new_v4().simple().to_string();
        Self(s[..8].to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EntryId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub model: String,
    pub system_prompt: String,
    /// Optional parent session — populated when this session is a fork.
    /// Pi calls this `parentSession`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session: Option<SessionId>,
    pub format_version: u32,
}

const FORMAT_VERSION: u32 = 2;

impl SessionHeader {
    pub fn new(
        session_id: SessionId,
        model: impl Into<String>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            created_at: Utc::now(),
            model: model.into(),
            system_prompt: system_prompt.into(),
            parent_session: None,
            format_version: FORMAT_VERSION,
        }
    }
}

/// One JSONL line. Every variant after `Header` carries `id` +
/// `parent_id` + `recorded_at` so the on-disk log is a tree by
/// construction. Replay walks parent pointers; the in-memory cursor
/// (`leaf_id`) is itself derived by replaying the most recent
/// `LeafChange` entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEntry {
    Header(SessionHeader),
    /// A canonical, fully-formed message appended to the conversation.
    Message {
        id: EntryId,
        #[serde(default)]
        parent_id: Option<EntryId>,
        recorded_at: DateTime<Utc>,
        seq: u64,
        message: AgentMessage,
    },
    /// Lifecycle event (turn boundaries, tool execution, etc.). Not on
    /// the model-context path; recorded for downstream observers reading
    /// the log directly.
    Event {
        id: EntryId,
        #[serde(default)]
        parent_id: Option<EntryId>,
        recorded_at: DateTime<Utc>,
        seq: u64,
        event: AgentEvent,
    },
    /// Cursor move. The most recent `LeafChange.to` is the current leaf.
    /// Linear sessions emit one of these after every `Message`. 032b
    /// adds caller-driven moves (rewind / fork).
    LeafChange {
        id: EntryId,
        #[serde(default)]
        parent_id: Option<EntryId>,
        recorded_at: DateTime<Utc>,
        from: Option<EntryId>,
        to: Option<EntryId>,
    },
}

impl SessionEntry {
    /// Entry id for non-header variants. Header has no id (it's the
    /// session-level metadata, not a tree node).
    pub fn id(&self) -> Option<&EntryId> {
        match self {
            Self::Header(_) => None,
            Self::Message { id, .. } | Self::Event { id, .. } | Self::LeafChange { id, .. } => {
                Some(id)
            }
        }
    }

    pub fn parent_id(&self) -> Option<&EntryId> {
        match self {
            Self::Header(_) => None,
            Self::Message { parent_id, .. }
            | Self::Event { parent_id, .. }
            | Self::LeafChange { parent_id, .. } => parent_id.as_ref(),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod sessions_tests;
