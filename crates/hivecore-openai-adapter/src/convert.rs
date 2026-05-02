//! Mappers between hivecore Layer 1 types and OpenAI wire types.

use hivecore_runtime_core::{
    AgentMessage, ContentBlock, MessageId, ModelChunk, ModelRequest, RuntimeError, RuntimeResult,
    StopReason, TokenUsage, ToolCallId,
};

use crate::error::OpenAiError;
use crate::wire::{
    ChatDelta, ChatMessage, ChatRequest, ChatStreamEvent, FinishReason, FunctionSpec,
    StreamOptions, ToolCall, ToolCallFunction, ToolSpec, Usage, UserContent, UserContentPart,
};

// ---- request ------------------------------------------------------------

pub fn build_request(req: &ModelRequest, stream: bool) -> Result<ChatRequest, OpenAiError> {
    let mut messages = Vec::with_capacity(req.messages.len() + 1);
    if !req.system.is_empty() {
        messages.push(ChatMessage::System {
            content: req.system.clone(),
        });
    }
    for m in &req.messages {
        if !m.visible_to_model() {
            continue;
        }
        messages.extend(message_to_wire(m)?);
    }

    let tools = if req.tools.is_empty() {
        None
    } else {
        Some(
            req.tools
                .iter()
                .map(|t| ToolSpec {
                    kind: "function".into(),
                    function: FunctionSpec {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect(),
        )
    };

    Ok(ChatRequest {
        model: req.model.clone(),
        messages,
        tools,
        tool_choice: None,
        max_completion_tokens: req.max_tokens,
        temperature: None,
        stream,
        stream_options: stream.then_some(StreamOptions {
            include_usage: true,
        }),
    })
}

fn message_to_wire(m: &AgentMessage) -> Result<Vec<ChatMessage>, OpenAiError> {
    Ok(match m {
        AgentMessage::User { content, .. } => vec![ChatMessage::User {
            content: user_content_to_wire(content),
        }],
        AgentMessage::Assistant { content, .. } => {
            let mut text = String::new();
            let mut tool_calls = Vec::new();
            for block in content {
                match block {
                    ContentBlock::Text { text: t } => text.push_str(t),
                    ContentBlock::ToolUse { id, name, input } => tool_calls.push(ToolCall {
                        id: id.0.clone(),
                        kind: "function".into(),
                        function: ToolCallFunction {
                            name: name.clone(),
                            arguments: input.to_string(),
                        },
                    }),
                    ContentBlock::Thinking { .. } | ContentBlock::Image { .. } => {
                        // Chat Completions has no thinking channel; images
                        // belong on user turns. Drop silently.
                    }
                }
            }
            vec![ChatMessage::Assistant {
                content: (!text.is_empty()).then_some(text),
                tool_calls,
            }]
        }
        AgentMessage::ToolResult {
            tool_call_id,
            content,
            ..
        } => vec![ChatMessage::Tool {
            tool_call_id: tool_call_id.0.clone(),
            content: text_of(content),
        }],
        AgentMessage::Custom {
            visible_to_model, ..
        } => {
            // Filtered earlier; defensively drop any that slipped through.
            debug_assert!(!*visible_to_model);
            vec![]
        }
    })
}

fn user_content_to_wire(blocks: &[ContentBlock]) -> UserContent {
    let only_text = blocks
        .iter()
        .all(|b| matches!(b, ContentBlock::Text { .. }));
    if only_text {
        return UserContent::Text(text_of(blocks));
    }
    let parts = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(UserContentPart::Text { text: text.clone() }),
            ContentBlock::Image { media_type, data } => {
                let url = format!("data:{media_type};base64,{data}");
                Some(UserContentPart::ImageUrl {
                    image_url: crate::wire::ImageUrl { url },
                })
            }
            _ => None,
        })
        .collect();
    UserContent::Parts(parts)
}

fn text_of(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

// ---- streaming response → ModelChunk ------------------------------------

/// Stateful aggregator for OpenAI streaming chunks. OpenAI splits tool calls
/// across many delta frames (id+name in the first, arguments in subsequent
/// frames keyed by `index`); the runtime expects whole `ToolUse` blocks.
#[derive(Debug, Default)]
pub struct StreamAggregator {
    message_id: Option<MessageId>,
    started: bool,
    tool_calls: Vec<PartialToolCall>,
    /// Set when a `finish_reason` arrives. We hold it until the usage frame
    /// (or [DONE]) is seen so `MessageEnd` carries real token counts.
    pending_finish: Option<StopReason>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl StreamAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Translate one OpenAI chunk into zero or more hivecore `ModelChunk`s.
    pub fn ingest(&mut self, ev: ChatStreamEvent) -> RuntimeResult<Vec<ModelChunk>> {
        let mut out = Vec::new();
        if !self.started {
            let id = MessageId(ev.id.clone());
            self.message_id = Some(id.clone());
            self.started = true;
            out.push(ModelChunk::MessageStart { id });
        }
        let id = self.message_id.clone().expect("message_id set");

        for choice in ev.choices {
            self.absorb_delta(&id, choice.delta, &mut out);
            if let Some(reason) = choice.finish_reason {
                self.flush_tool_calls(&id, &mut out);
                self.pending_finish = Some(stop_reason_from(reason));
            }
        }

        if let (Some(stop), Some(usage)) = (self.pending_finish, ev.usage.as_ref()) {
            out.push(ModelChunk::MessageEnd {
                id,
                stop_reason: stop,
                usage: usage_to_runtime(usage),
            });
            self.pending_finish = None;
        }

        Ok(out)
    }

    /// Called when the SSE stream signals `[DONE]`. If a finish was seen but
    /// no usage chunk arrived, emit `MessageEnd` with zero usage.
    pub fn finalize(&mut self) -> Option<ModelChunk> {
        let stop = self.pending_finish.take()?;
        let id = self.message_id.clone()?;
        Some(ModelChunk::MessageEnd {
            id,
            stop_reason: stop,
            usage: TokenUsage::default(),
        })
    }

    fn absorb_delta(&mut self, id: &MessageId, delta: ChatDelta, out: &mut Vec<ModelChunk>) {
        if let Some(text) = delta.reasoning_content {
            if !text.is_empty() {
                out.push(ModelChunk::ContentDelta {
                    id: id.clone(),
                    delta: ContentBlock::Thinking { text },
                });
            }
        }
        if let Some(text) = delta.content {
            if !text.is_empty() {
                out.push(ModelChunk::ContentDelta {
                    id: id.clone(),
                    delta: ContentBlock::Text { text },
                });
            }
        }
        if let Some(tool_deltas) = delta.tool_calls {
            for td in tool_deltas {
                let idx = td.index as usize;
                if self.tool_calls.len() <= idx {
                    self.tool_calls.resize_with(idx + 1, Default::default);
                }
                let slot = &mut self.tool_calls[idx];
                if let Some(new_id) = td.id {
                    slot.id = new_id;
                }
                if let Some(f) = td.function {
                    if let Some(name) = f.name {
                        slot.name.push_str(&name);
                    }
                    if let Some(args) = f.arguments {
                        slot.arguments.push_str(&args);
                    }
                }
            }
        }
    }

    fn flush_tool_calls(&mut self, id: &MessageId, out: &mut Vec<ModelChunk>) {
        for tc in std::mem::take(&mut self.tool_calls) {
            if tc.id.is_empty() && tc.name.is_empty() {
                continue;
            }
            let input = if tc.arguments.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_json::from_str(&tc.arguments)
                    .unwrap_or(serde_json::Value::String(tc.arguments.clone()))
            };
            out.push(ModelChunk::ContentDelta {
                id: id.clone(),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId(tc.id),
                    name: tc.name,
                    input,
                },
            });
        }
    }
}

fn stop_reason_from(reason: FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::EndTurn,
        FinishReason::Length => StopReason::MaxTokens,
        FinishReason::ToolCalls | FinishReason::FunctionCall => StopReason::ToolUse,
        FinishReason::ContentFilter => StopReason::Refusal,
    }
}

fn usage_to_runtime(u: &Usage) -> TokenUsage {
    TokenUsage {
        input: u.prompt_tokens,
        output: u.completion_tokens,
        cache_read: u
            .prompt_tokens_details
            .as_ref()
            .map(|d| d.cached_tokens)
            .unwrap_or(0),
        cache_write: 0,
    }
}

// Shut up unused-import lint when no callers reach for it (the alias keeps
// the public error type discoverable from this module).
#[allow(dead_code)]
fn _runtime_error_alias() -> RuntimeError {
    RuntimeError::Other(String::new())
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
