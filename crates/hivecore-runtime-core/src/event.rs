//! Lifecycle event taxonomy. Layer 2 subscribers (audit, websocket bridge, KG
//! ingest) consume `AgentEvent`s; the runtime never performs I/O itself.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{MessageId, SessionId, ToolCallId, TurnId};
use crate::message::{AgentMessage, ContentBlock, StopReason};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    AgentStart {
        session_id: SessionId,
        at: DateTime<Utc>,
    },
    AgentEnd {
        session_id: SessionId,
        at: DateTime<Utc>,
        reason: StopReason,
    },
    TurnStart {
        turn_id: TurnId,
        at: DateTime<Utc>,
    },
    TurnEnd {
        turn_id: TurnId,
        at: DateTime<Utc>,
        stop_reason: StopReason,
    },
    MessageStart {
        message_id: MessageId,
        turn_id: TurnId,
    },
    MessageDelta {
        message_id: MessageId,
        delta: ContentBlock,
    },
    MessageEnd {
        message_id: MessageId,
    },
    /// Emitted after a fully-formed message is appended to the session log.
    /// Persistence sinks rely on this to capture the canonical message, so
    /// replay can rebuild `AgentState` without reassembling deltas.
    MessageCommitted {
        message: AgentMessage,
    },
    ToolExecStart {
        tool_call_id: ToolCallId,
        name: String,
        input: serde_json::Value,
    },
    ToolExecUpdate {
        tool_call_id: ToolCallId,
        update: serde_json::Value,
    },
    ToolExecEnd {
        tool_call_id: ToolCallId,
        is_error: bool,
        result: serde_json::Value,
    },
    Custom {
        kind: String,
        payload: serde_json::Value,
    },
}
