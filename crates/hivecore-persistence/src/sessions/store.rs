//! `SessionStore` trait — read+write contract over a tree-shaped
//! conversation log (ADR-032c). The default impl is `SessionWriter` (in
//! `writer.rs`); future backends — sqlite, durable-objects, postgres,
//! s3 — implement the same trait and drop into the agent loop without
//! touching `hivecore-agent-loop` or `hivecore-compaction`.
//!
//! v0.1 keeps a single in-memory snapshot per store (the file is
//! re-read once on resume, then cached). Sessions stay small for v0.1
//! and the snapshot fits comfortably; ADR-032d will add lazy-load
//! variants if a real workload pushes back.

use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hivecore_runtime_core::{RuntimeError, SessionId};

use crate::error::PersistenceError;
use crate::sessions::{EntryId, SessionEntry};

#[derive(Debug, Clone)]
pub struct SessionMetadata {
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub model: String,
    pub system_prompt: String,
    pub parent_session: Option<SessionId>,
    pub format_version: u32,
    pub path: std::path::PathBuf,
}

/// Discriminator over `SessionEntry` variants. Match the `kind` tag
/// `SessionEntry` uses on the wire so callers stay decoupled from the
/// rust enum shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEntryKind {
    Header,
    Message,
    Event,
    LeafChange,
}

impl SessionEntryKind {
    pub fn matches(self, entry: &SessionEntry) -> bool {
        matches!(
            (self, entry),
            (Self::Header, SessionEntry::Header(_))
                | (Self::Message, SessionEntry::Message { .. })
                | (Self::Event, SessionEntry::Event { .. })
                | (Self::LeafChange, SessionEntry::LeafChange { .. })
        )
    }
}

/// Read+write contract for a session log. Object-safe; consumers store
/// `Arc<dyn SessionStore>`.
#[async_trait]
pub trait SessionStore: Send + Sync + std::fmt::Debug {
    async fn metadata(&self) -> Result<SessionMetadata, PersistenceError>;

    async fn leaf_id(&self) -> Result<Option<EntryId>, PersistenceError>;

    /// Move the cursor without writing a message. Internally appends a
    /// `LeafChange` entry so the move is replay-deterministic. Returns
    /// the id of the LeafChange entry.
    async fn set_leaf_id(&self, target: Option<EntryId>) -> Result<EntryId, PersistenceError>;

    async fn append_entry(&self, entry: SessionEntry) -> Result<(), PersistenceError>;

    async fn get_entry(&self, id: &EntryId) -> Result<Option<SessionEntry>, PersistenceError>;

    async fn find_entries_of_kind(
        &self,
        kind: SessionEntryKind,
    ) -> Result<Vec<SessionEntry>, PersistenceError>;

    /// Walk parent pointers from `leaf` to root. Root-first chronological
    /// order. `None` ⇒ use current leaf.
    async fn path_to_root(
        &self,
        leaf: Option<EntryId>,
    ) -> Result<Vec<SessionEntry>, PersistenceError>;

    /// All entries in append order.
    async fn all_entries(&self) -> Result<Vec<SessionEntry>, PersistenceError>;

    /// Forward index — children of `id` (entries whose `parent_id == id`).
    /// Returns ids only; callers fetch full entries via `get_entry` when
    /// needed.
    async fn children_of(&self, id: &EntryId) -> Result<Vec<EntryId>, PersistenceError>;

    /// Path on disk if the impl is filesystem-backed; `None` for
    /// in-memory / database / remote impls.
    fn path(&self) -> Option<&Path> {
        None
    }
}

/// Convenience: convert a `RuntimeError` from the agent loop into a
/// `PersistenceError` when bridging the SessionStore trait into a
/// runtime-shaped surface (e.g. agent-loop's resume builder).
pub fn runtime_to_persistence(e: RuntimeError) -> PersistenceError {
    PersistenceError::Invalid(e.to_string())
}
