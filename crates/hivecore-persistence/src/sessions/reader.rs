//! Session JSONL reader. Streams entries off disk; `into_messages()` filters
//! to the canonical message timeline (skipping per-delta events).

use std::path::Path;

use hivecore_runtime_core::AgentMessage;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{PersistenceError, Result};
use crate::sessions::{SessionEntry, SessionHeader};

#[derive(Debug)]
pub struct SessionReader {
    pub header: SessionHeader,
    pub entries: Vec<SessionEntry>,
}

impl SessionReader {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = tokio::fs::File::open(path).await?;
        let mut lines = BufReader::new(file).lines();
        let mut entries = Vec::new();
        let mut header: Option<SessionHeader> = None;

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                continue;
            }
            let entry: SessionEntry = serde_json::from_str(&line)?;
            match (&entry, &mut header) {
                (SessionEntry::Header(h), None) => header = Some(h.clone()),
                (SessionEntry::Header(_), Some(_)) => {
                    return Err(PersistenceError::Invalid("duplicate header line".into()))
                }
                _ => {}
            }
            entries.push(entry);
        }

        let header = header.ok_or_else(|| PersistenceError::Invalid("missing header".into()))?;
        Ok(Self { header, entries })
    }

    /// Canonical message timeline, in append order.
    pub fn messages(&self) -> Vec<AgentMessage> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                SessionEntry::Message { message, .. } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn event_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, SessionEntry::Event { .. }))
            .count()
    }
}
