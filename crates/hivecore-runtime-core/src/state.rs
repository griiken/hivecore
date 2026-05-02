//! Agent state snapshot. Plain data; serialization is the wire contract for
//! Layer 2/3 persistence and replay.

use serde::{Deserialize, Serialize};

use crate::ids::SessionId;
use crate::message::AgentMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    Off,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    /// JSON Schema describing accepted arguments.
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub session_id: SessionId,
    pub system_prompt: String,
    pub model: String,
    pub thinking: ThinkingLevel,
    pub tools: Vec<ToolDescriptor>,
    pub messages: Vec<AgentMessage>,
    pub is_streaming: bool,
}
