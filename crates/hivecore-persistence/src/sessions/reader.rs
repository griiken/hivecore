//! Session JSONL reader (ADR-032 schema). Streams entries off disk and
//! exposes both raw entries and the canonical model-context message
//! timeline derived by walking parent pointers from the current leaf.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use hivecore_runtime_core::AgentMessage;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{PersistenceError, Result};
use crate::sessions::{EntryId, SessionEntry, SessionHeader};

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

    /// Current leaf id derived from the most recent `LeafChange.to`. If
    /// no LeafChange entries exist yet, falls back to the most recent
    /// `Message.id` — handles half-written files where a Message landed
    /// but its trailing LeafChange did not.
    pub fn leaf_id(&self) -> Option<EntryId> {
        for entry in self.entries.iter().rev() {
            if let SessionEntry::LeafChange { to, .. } = entry {
                return to.clone();
            }
        }
        for entry in self.entries.iter().rev() {
            if let SessionEntry::Message { id, .. } = entry {
                return Some(id.clone());
            }
        }
        None
    }

    /// All entry ids issued so far. Writers reopening the file pass this
    /// to `SessionWriter::open_append_with_state` to preserve the
    /// collision-guard invariant across restarts.
    pub fn issued_ids(&self) -> HashSet<EntryId> {
        let mut out = HashSet::new();
        for entry in &self.entries {
            if let Some(id) = entry.id() {
                out.insert(id.clone());
            }
        }
        out
    }

    /// Walk parent pointers from `leaf` to root, gathering only `Message`
    /// payloads in chronological (root-first) order. This is the model-
    /// context derivation per ADR-032 §Replay. For 032a, linear sessions
    /// have a single chain and this returns the same list `messages()`
    /// returned under the ADR-026 schema.
    pub fn path_to_root(&self, leaf: Option<EntryId>) -> Vec<AgentMessage> {
        let by_id: HashMap<&EntryId, &SessionEntry> = self
            .entries
            .iter()
            .filter_map(|e| e.id().map(|id| (id, e)))
            .collect();

        // Walk from leaf to root following parent_id pointers.
        let mut chain: Vec<&SessionEntry> = Vec::new();
        let mut cursor = leaf;
        let mut guard = 0usize;
        while let Some(id) = cursor.take() {
            guard += 1;
            if guard > self.entries.len() + 8 {
                break; // bail on cycle
            }
            let Some(entry) = by_id.get(&id) else {
                break;
            };
            chain.push(entry);
            cursor = entry.parent_id().cloned();
        }
        chain.reverse();
        chain
            .into_iter()
            .filter_map(|e| match e {
                SessionEntry::Message { message, .. } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    /// Canonical message timeline from the current leaf. Use
    /// `path_to_root(None)` to derive context for some other point in
    /// the tree (032b).
    pub fn messages(&self) -> Vec<AgentMessage> {
        self.path_to_root(self.leaf_id())
    }

    pub fn event_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, SessionEntry::Event { .. }))
            .count()
    }
}
