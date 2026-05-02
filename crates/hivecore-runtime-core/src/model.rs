//! LLM provider contract. Concrete adapters live in sibling spikes / crates
//! (e.g. `spikes/openai-adapter`); Layer 1 defines only the trait + DTOs.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::abort::AbortSignal;
use crate::error::RuntimeResult;
use crate::ids::MessageId;
use crate::message::AgentMessage;
use crate::message::{ContentBlock, StopReason};
use crate::state::{ThinkingLevel, ToolDescriptor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<ToolDescriptor>,
    pub thinking: ThinkingLevel,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelChunk {
    MessageStart {
        id: MessageId,
    },
    ContentDelta {
        id: MessageId,
        delta: ContentBlock,
    },
    MessageEnd {
        id: MessageId,
        stop_reason: StopReason,
        usage: TokenUsage,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
}

pub type ModelStream = Pin<Box<dyn Stream<Item = RuntimeResult<ModelChunk>> + Send>>;

#[async_trait]
pub trait ModelAdapter: Send + Sync {
    fn id(&self) -> &str;
    async fn complete(&self, req: ModelRequest, signal: AbortSignal) -> RuntimeResult<ModelStream>;
}
