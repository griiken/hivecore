//! Conversation message types.
//!
//! `AgentMessage::Custom` mirrors pi's `appendEntry` — extension state that is
//! persisted to the session log but optionally hidden from the LLM.

use serde::{Deserialize, Serialize};

use crate::ids::{MessageId, ToolCallId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum AgentMessage {
    User {
        id: MessageId,
        content: Vec<ContentBlock>,
    },
    Assistant {
        id: MessageId,
        content: Vec<ContentBlock>,
        stop_reason: Option<StopReason>,
    },
    ToolResult {
        id: MessageId,
        tool_call_id: ToolCallId,
        content: Vec<ContentBlock>,
        is_error: bool,
    },
    Custom {
        id: MessageId,
        kind: String,
        payload: serde_json::Value,
        /// `false` ⇒ persisted but never sent to the model.
        visible_to_model: bool,
    },
}

impl AgentMessage {
    pub fn id(&self) -> &MessageId {
        match self {
            Self::User { id, .. }
            | Self::Assistant { id, .. }
            | Self::ToolResult { id, .. }
            | Self::Custom { id, .. } => id,
        }
    }

    pub fn visible_to_model(&self) -> bool {
        match self {
            Self::Custom {
                visible_to_model, ..
            } => *visible_to_model,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: ToolCallId,
        name: String,
        input: serde_json::Value,
    },
    Thinking {
        text: String,
    },
    Image {
        media_type: String,
        data: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    ToolUse,
    StopSequence,
    Refusal,
    Error,
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
