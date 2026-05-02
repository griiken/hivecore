//! Stream accumulator. Folds a `ModelStream` of `ModelChunk`s into a single
//! `AgentMessage::Assistant` plus a stop reason and usage. The loop never
//! sees raw chunks — they go to the event sink while this builds the message.

use async_trait::async_trait;
use futures::StreamExt;
use hivecore_runtime_core::{
    AgentEvent, AgentMessage, ContentBlock, MessageId, ModelChunk, ModelStream, RuntimeError,
    StopReason, TokenUsage, ToolCallId, TurnId,
};

use crate::sink::EventSink;

#[derive(Debug, Clone)]
pub struct AssembledTurn {
    pub message: AgentMessage,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
    pub tool_calls: Vec<ToolCallRequest>,
}

#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub id: ToolCallId,
    pub name: String,
    pub input: serde_json::Value,
}

#[async_trait]
pub trait StreamConsumer {
    async fn consume(
        &self,
        turn_id: TurnId,
        stream: ModelStream,
    ) -> Result<AssembledTurn, RuntimeError>;
}

pub struct DefaultConsumer<'a> {
    pub sink: &'a dyn EventSink,
}

impl std::fmt::Debug for DefaultConsumer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultConsumer").finish()
    }
}

#[async_trait]
impl StreamConsumer for DefaultConsumer<'_> {
    async fn consume(
        &self,
        turn_id: TurnId,
        mut stream: ModelStream,
    ) -> Result<AssembledTurn, RuntimeError> {
        let mut id: Option<MessageId> = None;
        let mut text = String::new();
        let mut thinking = String::new();
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();
        let mut stop_reason = StopReason::EndTurn;
        let mut usage = TokenUsage::default();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            match chunk {
                ModelChunk::MessageStart { id: mid } => {
                    id = Some(mid.clone());
                    self.sink
                        .emit(AgentEvent::MessageStart {
                            message_id: mid,
                            turn_id,
                        })
                        .await;
                }
                ModelChunk::ContentDelta { id: mid, delta } => {
                    match &delta {
                        ContentBlock::Text { text: t } => text.push_str(t),
                        ContentBlock::Thinking { text: t } => thinking.push_str(t),
                        ContentBlock::ToolUse {
                            id: tid,
                            name,
                            input,
                        } => tool_calls.push(ToolCallRequest {
                            id: tid.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        }),
                        ContentBlock::Image { .. } => {}
                    }
                    self.sink
                        .emit(AgentEvent::MessageDelta {
                            message_id: mid,
                            delta,
                        })
                        .await;
                }
                ModelChunk::MessageEnd {
                    id: mid,
                    stop_reason: reason,
                    usage: u,
                } => {
                    stop_reason = reason;
                    usage = u;
                    self.sink
                        .emit(AgentEvent::MessageEnd { message_id: mid })
                        .await;
                }
            }
        }

        let mid = id.unwrap_or_else(|| MessageId(String::from("synthetic")));
        let mut content = Vec::new();
        if !thinking.is_empty() {
            content.push(ContentBlock::Thinking { text: thinking });
        }
        if !text.is_empty() {
            content.push(ContentBlock::Text { text });
        }
        for tc in &tool_calls {
            content.push(ContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: tc.input.clone(),
            });
        }

        let message = AgentMessage::Assistant {
            id: mid,
            content,
            stop_reason: Some(stop_reason),
        };

        Ok(AssembledTurn {
            message,
            stop_reason,
            usage,
            tool_calls,
        })
    }
}

#[cfg(test)]
#[path = "accumulator_tests.rs"]
mod tests;
