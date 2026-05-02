//! `SummarizingTransform` — `ContextTransform` that compacts via summarization.
//!
//! Two responsibilities (ADR-026):
//! - **`maybe_compact(&state)`** — token-threshold check; if over, calls
//!   summarizer model on the head, builds + returns a `compaction_marker`
//!   message. The driver appends it to `state.messages` for persistence.
//! - **`transform_outgoing(&state, messages)`** — derives the model payload
//!   by walking newest→oldest to the latest marker. Replacement layout
//!   (Codex shape, ADR-026 §Decisions):
//!   `[real user messages from head] + [synthetic-assistant(summary)] + [kept tail]`
//!
//! Multiple markers stack: the LATEST marker wins; older markers + their
//! source messages are entirely replaced by the latest summary.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use hivecore_runtime_core::{
    AbortSignal, AgentMessage, AgentState, ContentBlock, ContextTransform, MessageId, ModelAdapter,
    ModelChunk, ModelRequest, RuntimeError, RuntimeResult, ThinkingLevel,
};
use tokio::sync::Mutex;

use crate::constants::CompactionConfig;
use crate::marker::{CompactionMarker, CompactionTrigger, FileRef};
use crate::prompts::{SUMMARY_PREFIX, SUMMARY_PROMPT};
use crate::token::estimate_tokens;

/// `ContextTransform` impl. Cheap to clone; share via `Arc`.
pub struct SummarizingTransform {
    summarizer: Arc<dyn ModelAdapter>,
    summarizer_model_id: String,
    config: CompactionConfig,
    /// Guards reentrancy — multiple concurrent `maybe_compact` calls would
    /// each fire a model call; serialise them. In practice the driver only
    /// calls once per turn so contention is rare.
    busy: Arc<Mutex<()>>,
}

impl std::fmt::Debug for SummarizingTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SummarizingTransform")
            .field("summarizer_model_id", &self.summarizer_model_id)
            .field("config", &self.config)
            .finish()
    }
}

impl SummarizingTransform {
    pub fn new(
        summarizer: Arc<dyn ModelAdapter>,
        summarizer_model_id: impl Into<String>,
        config: CompactionConfig,
    ) -> Self {
        Self {
            summarizer,
            summarizer_model_id: summarizer_model_id.into(),
            config,
            busy: Arc::new(Mutex::new(())),
        }
    }

    /// Pick the boundary between "head to summarize" and "kept tail".
    /// Walks backward from the end keeping `recent_turns_to_keep` user/assistant
    /// turns intact. Returns `(covers_up_to, kept_tail_start, head_indices)`.
    fn pick_cut(&self, messages: &[AgentMessage]) -> Option<CutPoint> {
        let mut turn_count = 0usize;
        let mut tail_start_idx = messages.len();
        for (i, m) in messages.iter().enumerate().rev() {
            if matches!(
                m,
                AgentMessage::User { .. } | AgentMessage::Assistant { .. }
            ) {
                turn_count += 1;
                if turn_count >= self.config.recent_turns_to_keep {
                    tail_start_idx = i;
                    break;
                }
            }
        }
        // We never moved tail_start_idx below messages.len() — not enough
        // user/assistant turns to satisfy recent_turns_to_keep. Skip.
        if tail_start_idx >= messages.len() || tail_start_idx == 0 {
            return None;
        }
        // Honour MIN_TURNS_TO_KEEP — never strip below the floor.
        if messages.len() <= self.config.min_turns_to_keep {
            return None;
        }
        let head = &messages[..tail_start_idx];
        // covers_up_to = id of last visible message in head.
        let covers_up_to = head.iter().rev().find_map(|m| {
            if m.visible_to_model() {
                Some(m.id().clone())
            } else {
                None
            }
        })?;
        let kept_tail_start = messages[tail_start_idx].id().clone();
        Some(CutPoint {
            covers_up_to,
            kept_tail_start,
            head_count: tail_start_idx,
        })
    }

    /// Render head messages into a single user-text body for the
    /// summarizer model. Tool calls + results are collapsed into a
    /// human-readable form.
    fn serialize_head(&self, head: &[AgentMessage]) -> String {
        let mut out = String::new();
        for m in head {
            if !m.visible_to_model() {
                continue;
            }
            match m {
                AgentMessage::User { content, .. } => {
                    out.push_str("USER: ");
                    out.push_str(&plain_text(content));
                    out.push('\n');
                }
                AgentMessage::Assistant { content, .. } => {
                    let text = plain_text(content);
                    if !text.is_empty() {
                        out.push_str("ASSISTANT: ");
                        out.push_str(&text);
                        out.push('\n');
                    }
                    for b in content {
                        if let ContentBlock::ToolUse { name, input, .. } = b {
                            out.push_str(&format!("TOOL_CALL {name} {input}\n"));
                        }
                    }
                }
                AgentMessage::ToolResult {
                    content, is_error, ..
                } => {
                    let tag = if *is_error {
                        "TOOL_ERROR"
                    } else {
                        "TOOL_RESULT"
                    };
                    let mut text = plain_text(content);
                    if text.len() > self.config.emergency_tool_result_max_chars {
                        text.truncate(self.config.emergency_tool_result_max_chars);
                        text.push_str("…[truncated]");
                    }
                    out.push_str(&format!("{tag}: {text}\n"));
                }
                AgentMessage::Custom { .. } => {}
            }
        }
        out
    }

    fn extract_file_refs(&self, head: &[AgentMessage]) -> Vec<FileRef> {
        let mut refs = Vec::new();
        for m in head {
            if let AgentMessage::Assistant { content, .. } = m {
                for b in content {
                    if let ContentBlock::ToolUse { name, input, .. } = b {
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            let op = match name.as_str() {
                                "read_file" => "read",
                                "write_file" => "write",
                                "edit_file" => "edit",
                                _ => continue,
                            };
                            refs.push(FileRef {
                                path: path.to_string(),
                                op: op.to_string(),
                            });
                        }
                    }
                }
            }
        }
        refs
    }

    async fn run_summarizer(&self, head_text: String) -> RuntimeResult<String> {
        let req = ModelRequest {
            model: self.summarizer_model_id.clone(),
            system: SUMMARY_PROMPT.to_string(),
            messages: vec![AgentMessage::User {
                id: MessageId(format!("compact-input-{}", uuid::Uuid::new_v4())),
                content: vec![ContentBlock::Text { text: head_text }],
            }],
            tools: Vec::new(),
            thinking: ThinkingLevel::Off,
            max_tokens: None,
        };
        let (_handle, signal) = AbortSignal::new();
        let mut stream = self.summarizer.complete(req, signal).await?;
        let mut text = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk? {
                ModelChunk::ContentDelta {
                    delta: ContentBlock::Text { text: t },
                    ..
                } => text.push_str(&t),
                ModelChunk::MessageEnd { .. } => break,
                _ => {}
            }
        }
        Ok(text)
    }
}

#[derive(Debug)]
struct CutPoint {
    covers_up_to: MessageId,
    kept_tail_start: MessageId,
    head_count: usize,
}

#[async_trait]
impl ContextTransform for SummarizingTransform {
    async fn maybe_compact(&self, state: &AgentState) -> RuntimeResult<Option<AgentMessage>> {
        let _guard = self.busy.lock().await;

        let est = estimate_tokens(&state.messages);
        if est < self.config.threshold_tokens() {
            return Ok(None);
        }
        let trigger = if est >= self.config.critical_tokens() {
            CompactionTrigger::Emergency
        } else {
            CompactionTrigger::AutoThreshold
        };
        tracing::info!(
            est,
            threshold = self.config.threshold_tokens(),
            critical = self.config.critical_tokens(),
            ?trigger,
            "compaction firing",
        );

        let cut = match self.pick_cut(&state.messages) {
            Some(c) => c,
            None => {
                tracing::warn!("compaction wanted but min_turns_to_keep blocked it");
                return Ok(None);
            }
        };
        let head = &state.messages[..cut.head_count];
        let head_text = self.serialize_head(head);
        let summary_text = self.run_summarizer(head_text).await?;
        if summary_text.trim().is_empty() {
            return Err(RuntimeError::Other("summarizer returned empty body".into()));
        }

        let tool_calls_summarized = head
            .iter()
            .map(|m| match m {
                AgentMessage::Assistant { content, .. } => content
                    .iter()
                    .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                    .count(),
                _ => 0,
            })
            .sum();
        let original_turn_count = head
            .iter()
            .filter(|m| {
                matches!(
                    m,
                    AgentMessage::User { .. } | AgentMessage::Assistant { .. }
                )
            })
            .count();

        let marker = CompactionMarker {
            covers_up_to: cut.covers_up_to,
            summary_text,
            kept_tail_start: cut.kept_tail_start,
            file_refs: self.extract_file_refs(head),
            tool_calls_summarized,
            original_turn_count,
            created_at: Utc::now(),
            trigger,
        };
        Ok(Some(marker.to_message()))
    }

    async fn transform_outgoing(
        &self,
        _state: &AgentState,
        messages: Vec<AgentMessage>,
    ) -> RuntimeResult<Vec<AgentMessage>> {
        // Find latest marker (newest → oldest).
        let mut latest_marker_idx: Option<usize> = None;
        let mut latest_marker: Option<CompactionMarker> = None;
        for (i, m) in messages.iter().enumerate().rev() {
            if let Some(mk) = CompactionMarker::try_from_message(m) {
                latest_marker_idx = Some(i);
                latest_marker = Some(mk);
                break;
            }
        }
        let (Some(marker_idx), Some(marker)) = (latest_marker_idx, latest_marker) else {
            // No compaction yet — pass through, dropping non-visible Customs.
            return Ok(messages
                .into_iter()
                .filter(|m| m.visible_to_model())
                .collect());
        };

        // Head = messages[..marker_idx]; keep only real user messages
        // (Codex pattern — preserve task statements / corrections).
        let mut out: Vec<AgentMessage> = messages[..marker_idx]
            .iter()
            .filter(|m| matches!(m, AgentMessage::User { .. }))
            .cloned()
            .collect();

        // Synthetic assistant message holding the summary + Codex preamble.
        out.push(AgentMessage::Assistant {
            id: MessageId(format!("compact-summary-{}", uuid::Uuid::new_v4())),
            content: vec![ContentBlock::Text {
                text: format!("{}\n\n{}", SUMMARY_PREFIX, marker.summary_text),
            }],
            stop_reason: None,
        });

        // Tail — everything after the marker, visible only.
        for m in &messages[marker_idx + 1..] {
            if m.visible_to_model() {
                out.push(m.clone());
            }
        }
        Ok(out)
    }
}

fn plain_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
