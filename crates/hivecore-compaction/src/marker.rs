//! `compaction_marker` — the structured payload appended to the session
//! log when compaction fires (ADR-026). Rides the existing
//! `AgentMessage::Custom { kind, payload, visible_to_model: false }`
//! channel so no `SessionEntry` schema change is needed.

use chrono::{DateTime, Utc};
use hivecore_runtime_core::{AgentMessage, MessageId};
use serde::{Deserialize, Serialize};

/// `Custom.kind` discriminator.
pub const MARKER_KIND: &str = "compaction_marker";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    Manual,
    AutoThreshold,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileRef {
    pub path: String,
    /// "read" | "write" | "edit"
    pub op: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionMarker {
    /// MessageId of the last message folded into the summary. Replay walks
    /// newest→oldest; entries with id ≤ this are skipped from the payload.
    pub covers_up_to: MessageId,

    pub summary_text: String,

    /// First message id kept verbatim in the payload tail.
    pub kept_tail_start: MessageId,

    /// Structural file payload — survives across compactions (pi pattern).
    /// Lets next-turn KG ingestion know what files prior turns touched.
    #[serde(default)]
    pub file_refs: Vec<FileRef>,

    pub tool_calls_summarized: usize,
    pub original_turn_count: usize,
    pub created_at: DateTime<Utc>,
    pub trigger: CompactionTrigger,
}

impl CompactionMarker {
    /// Wrap as a `Custom` message ready to push into `state.messages`.
    /// `visible_to_model = false` — the marker is metadata; the synthetic
    /// summary message inserted by `transform_outgoing` is what the model
    /// actually sees.
    pub fn to_message(&self) -> AgentMessage {
        let payload = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        AgentMessage::Custom {
            id: MessageId(format!("compact-{}", uuid::Uuid::new_v4())),
            kind: MARKER_KIND.to_string(),
            payload,
            visible_to_model: false,
        }
    }

    /// Read the marker payload back out of an `AgentMessage::Custom` if it
    /// is one. Returns `None` for any other shape or any custom kind that
    /// isn't `compaction_marker`.
    pub fn try_from_message(msg: &AgentMessage) -> Option<Self> {
        match msg {
            AgentMessage::Custom {
                kind,
                payload,
                visible_to_model: false,
                ..
            } if kind == MARKER_KIND => serde_json::from_value(payload.clone()).ok(),
            _ => None,
        }
    }
}
