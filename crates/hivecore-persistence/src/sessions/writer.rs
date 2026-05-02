//! Append-only session writer. Implements `EventSink` so it slots straight
//! into `AgentLoop::builder().sink(...)`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use hivecore_runtime_core::AgentEvent;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::sessions::{SessionEntry, SessionHeader};

/// Writes session JSONL. Cheap to clone; the underlying file handle is
/// shared via `Arc<Mutex<...>>` and serialised so writes never interleave.
#[derive(Clone)]
pub struct SessionWriter {
    path: Arc<PathBuf>,
    file: Arc<Mutex<tokio::fs::File>>,
    seq: Arc<AtomicU64>,
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
    /// a writer ready to absorb events. Use a directory + `<session_id>.jsonl`
    /// naming scheme at the call site.
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
        })
    }

    /// Open an existing session file for append. Caller is responsible for
    /// ensuring the header line is already present (i.e. the file was
    /// previously `create()`d). Used by `--continue` / `--session` resume
    /// paths in `hivecore-coder`. Seq counter resumes from zero — the
    /// driver doesn't read seqs back, so collisions are tolerable for v0.1.
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
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Append a fully-formed message. The agent loop emits these via
    /// `AgentEvent::MessageCommitted`; this method is exposed for callers
    /// that want to persist messages outside the loop (e.g. ingest tooling).
    pub async fn append_message(&self, message: hivecore_runtime_core::AgentMessage) -> Result<()> {
        let entry = SessionEntry::Message {
            seq: self.next_seq(),
            recorded_at: Utc::now(),
            message,
        };
        let mut f = self.file.lock().await;
        write_line(&mut f, &entry).await
    }

    pub async fn append_event(&self, event: AgentEvent) -> Result<()> {
        let entry = SessionEntry::Event {
            seq: self.next_seq(),
            recorded_at: Utc::now(),
            event,
        };
        let mut f = self.file.lock().await;
        write_line(&mut f, &entry).await
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }
}

#[async_trait]
impl hivecore_runtime_core::EventSink for SessionWriter {
    async fn emit(&self, event: AgentEvent) {
        // `MessageCommitted` is recorded as a Message entry so replay sees a
        // single canonical timeline. All other events are recorded verbatim.
        let result = match event.clone() {
            AgentEvent::MessageCommitted { message } => self.append_message(message).await,
            other => self.append_event(other).await,
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "session writer dropped event");
        }
    }
}

async fn write_line(f: &mut tokio::fs::File, entry: &SessionEntry) -> Result<()> {
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    f.write_all(line.as_bytes()).await?;
    f.flush().await?;
    Ok(())
}
