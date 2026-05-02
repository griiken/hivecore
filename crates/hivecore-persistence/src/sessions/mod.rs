//! Session JSONL — one file per `SessionId`. Append-only. Each line is a
//! `SessionEntry` (header / event / message). Replay reconstructs the
//! committed message stream.

pub mod index;
pub mod lock;
mod reader;
mod writer;

pub use index::{
    append_entry, find_by_id, find_by_name, find_latest_by_cwd, index_path, resolve_handle,
    SessionIndexEntry,
};
pub use lock::{acquire as acquire_lock, LockError, SessionLock};
pub use reader::SessionReader;
pub use writer::SessionWriter;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use hivecore_runtime_core::{AgentEvent, AgentMessage, SessionId};
use serde::{Deserialize, Serialize};

/// Build the canonical session JSONL path:
/// `<sessions_dir>/<tenant_id>/<session_id>.jsonl` (ADR-026).
pub fn session_path(sessions_dir: &Path, tenant_id: &str, session_id: SessionId) -> PathBuf {
    sessions_dir
        .join(tenant_id)
        .join(format!("{}.jsonl", session_id.0))
}

/// Build the per-tenant directory: `<sessions_dir>/<tenant_id>/`.
pub fn tenant_dir(sessions_dir: &Path, tenant_id: &str) -> PathBuf {
    sessions_dir.join(tenant_id)
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

const FORMAT_VERSION: u32 = 1;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEntry {
    Header(SessionHeader),
    /// A canonical, fully-formed message appended to the conversation.
    Message {
        seq: u64,
        recorded_at: DateTime<Utc>,
        message: AgentMessage,
    },
    /// Lifecycle event (turn boundaries, tool execution, etc.). Not strictly
    /// required for replay but useful for downstream observers reading the
    /// log directly.
    Event {
        seq: u64,
        recorded_at: DateTime<Utc>,
        event: AgentEvent,
    },
}

#[cfg(test)]
#[path = "tests.rs"]
mod sessions_tests;
