//! Append-only session writer (ADR-032 schema). Implements `EventSink` so
//! it slots into `AgentLoop::builder().sink(...)` unchanged.
//!
//! Every `Message` append also writes a `LeafChange` entry recording the
//! cursor move from the previous leaf to the new entry. Linear sessions
//! produce one LeafChange per Message; 032b adds caller-driven moves
//! (rewind, fork) which produce additional LeafChange entries without
//! any Message append.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use hivecore_runtime_core::AgentEvent;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::sessions::{EntryId, SessionEntry, SessionHeader};

/// Writes session JSONL. Cheap to clone; the underlying file handle and
/// in-memory cursor (`leaf` + id-set) are shared via `Arc<Mutex<...>>`
/// so writes from concurrent emitters never interleave on disk and the
/// id-collision guard sees every issued id.
#[derive(Clone)]
pub struct SessionWriter {
    path: Arc<PathBuf>,
    file: Arc<Mutex<tokio::fs::File>>,
    seq: Arc<AtomicU64>,
    state: Arc<Mutex<WriterState>>,
}

#[derive(Debug, Default)]
struct WriterState {
    leaf: Option<EntryId>,
    seen_ids: HashSet<EntryId>,
}

impl std::fmt::Debug for SessionWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionWriter")
            .field("path", &self.path)
            .finish()
    }
}

impl SessionWriter {
    /// Create or truncate the file at `path`, write the header, and return
    /// a writer ready to absorb events.
    pub async fn create(path: impl AsRef<Path>, header: SessionHeader) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;
        write_line(&mut file, &SessionEntry::Header(header)).await?;
        Ok(Self {
            path: Arc::new(path),
            file: Arc::new(Mutex::new(file)),
            seq: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(WriterState::default())),
        })
    }

    /// Open an existing session file for append. Caller is responsible for
    /// ensuring the header line is already present (i.e. the file was
    /// previously `create()`d). Used by `--continue` / `--session` resume
    /// paths in `hivecore-coder`.
    ///
    /// The leaf cursor + id-set are recovered by reading the file via
    /// `SessionReader::open` first; callers should construct a writer via
    /// `open_append_with_state` to preserve cursor continuity. The plain
    /// `open_append` returns a writer with no inherited cursor — fine for
    /// pure append-only callers (tests, simple migration), but
    /// `LeafChange.from` will be `None` until the first message lands.
    pub async fn open_append(path: impl AsRef<Path>) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();
        let file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await?;
        Ok(Self {
            path: Arc::new(path),
            file: Arc::new(Mutex::new(file)),
            seq: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(WriterState::default())),
        })
    }

    /// Open existing file for append, seeding cursor + id-set from a
    /// prior `SessionReader` snapshot. Resume path uses this so the next
    /// `LeafChange.from` correctly points at the last leaf on disk.
    pub async fn open_append_with_state(
        path: impl AsRef<Path>,
        leaf: Option<EntryId>,
        seen_ids: HashSet<EntryId>,
    ) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();
        let file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await?;
        Ok(Self {
            path: Arc::new(path),
            file: Arc::new(Mutex::new(file)),
            seq: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(WriterState { leaf, seen_ids })),
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Append a fully-formed message. Atomic from the agent loop's POV:
    /// emits the Message entry then a LeafChange entry, both lines
    /// flushed before the lock is released.
    pub async fn append_message(&self, message: hivecore_runtime_core::AgentMessage) -> Result<()> {
        let mut f = self.file.lock().await;
        let mut s = self.state.lock().await;
        let parent_id = s.leaf.clone();
        let id = next_unique_id(&mut s.seen_ids);
        let recorded_at = Utc::now();
        let seq = self.next_seq();
        write_line(
            &mut f,
            &SessionEntry::Message {
                id: id.clone(),
                parent_id: parent_id.clone(),
                recorded_at,
                seq,
                message,
            },
        )
        .await?;

        let from = parent_id;
        let to = Some(id.clone());
        let leaf_id = next_unique_id(&mut s.seen_ids);
        write_line(
            &mut f,
            &SessionEntry::LeafChange {
                id: leaf_id,
                parent_id: Some(id.clone()),
                recorded_at: Utc::now(),
                from,
                to: to.clone(),
            },
        )
        .await?;
        s.leaf = to;
        Ok(())
    }

    pub async fn append_event(&self, event: AgentEvent) -> Result<()> {
        let mut f = self.file.lock().await;
        let mut s = self.state.lock().await;
        let parent_id = s.leaf.clone();
        let id = next_unique_id(&mut s.seen_ids);
        write_line(
            &mut f,
            &SessionEntry::Event {
                id,
                parent_id,
                recorded_at: Utc::now(),
                seq: self.next_seq(),
                event,
            },
        )
        .await
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }
}

#[async_trait]
impl hivecore_runtime_core::EventSink for SessionWriter {
    async fn emit(&self, event: AgentEvent) {
        // `MessageCommitted` is recorded as a Message entry (+ trailing
        // LeafChange) so replay sees a single canonical conversation tree.
        // All other events are recorded as Event entries on the side.
        let result = match event.clone() {
            AgentEvent::MessageCommitted { message } => self.append_message(message).await,
            other => self.append_event(other).await,
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "session writer dropped event");
        }
    }
}

fn next_unique_id(seen: &mut HashSet<EntryId>) -> EntryId {
    for _ in 0..32 {
        let id = EntryId::new();
        if seen.insert(id.clone()) {
            return id;
        }
    }
    // Fallback: include extra entropy by extending to 16 chars.
    let s = uuid::Uuid::new_v4().simple().to_string();
    let id = EntryId(s[..16].to_string());
    seen.insert(id.clone());
    id
}

async fn write_line(f: &mut tokio::fs::File, entry: &SessionEntry) -> Result<()> {
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    f.write_all(line.as_bytes()).await?;
    f.flush().await?;
    Ok(())
}
