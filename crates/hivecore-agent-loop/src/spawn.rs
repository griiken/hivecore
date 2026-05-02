//! `SpawnAgentTool` — sub-agent recursion as a first-class `Tool`.
//!
//! Per `.planning/intel/skills-subagents-survey.md` decision matrix:
//! sub-agent spawning is a *substrate* primitive — every harness needs it.
//! Implementation pattern borrowed from `openai/codex/codex_delegate.rs`:
//! the child inherits the parent's services (model adapter, sink, hooks)
//! and runs to completion; cancellation cascades.
//!
//! v0.1 scope (this spike):
//!   - Single-level (no recursion through SpawnAgentTool itself; depth=1).
//!   - Inherits parent's `ModelAdapter`, `EventSink`, hooks.
//!   - Independent `SessionId`, `AgentState`.
//!   - Caller picks the child's tool registry + system prompt at construction.
//!
//! Deferred (v0.2):
//!   - Persona lookup (caller supplies a closure today).
//!   - `MAX_SUBAGENT_DEPTH` enforcement (Zed pattern).
//!   - Streaming child events back as parent `ToolExecUpdate`.

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, AgentMessage, ContentBlock, EventSink, MessageId, ModelAdapter, RuntimeError,
    RuntimeResult, SessionId, ThinkingLevel, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;

use crate::driver::{user_text, AgentLoop};
use crate::registry::ToolRegistry;

/// Builds the child loop on demand. The spawn tool calls this once per
/// invocation to get a fresh `AgentLoop`.
pub trait SubAgentSpec: Send + Sync + std::fmt::Debug {
    /// Identifier (passed to `SpawnAgentTool` as `persona` arg). Used to
    /// look up *which* spec to instantiate when a registry is in play.
    fn name(&self) -> &str;

    /// Human-readable description shown to the model.
    fn description(&self) -> &str;

    /// Build the child agent loop. Implementations are responsible for
    /// inheriting any state they want from the parent (model, sink, etc.).
    fn build(&self) -> Result<AgentLoop, RuntimeError>;
}

/// Convenience: `SubAgentSpec` you assemble at construction site without
/// writing a struct.
pub struct ClosureSpec {
    name: String,
    description: String,
    builder: Box<dyn Fn() -> Result<AgentLoop, RuntimeError> + Send + Sync>,
}

impl ClosureSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        builder: impl Fn() -> Result<AgentLoop, RuntimeError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            builder: Box::new(builder),
        }
    }
}

impl std::fmt::Debug for ClosureSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosureSpec")
            .field("name", &self.name)
            .finish()
    }
}

impl SubAgentSpec for ClosureSpec {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn build(&self) -> Result<AgentLoop, RuntimeError> {
        (self.builder)()
    }
}

/// Tool exposed to the parent agent. The model invokes it with
/// `{persona: "<name>", prompt: "<task>"}`. We spawn a fresh `AgentLoop`,
/// drive it to its natural stop, and return the child's final assistant
/// text as the tool result.
pub struct SpawnAgentTool {
    specs: Vec<Arc<dyn SubAgentSpec>>,
    description_text: String,
}

impl SpawnAgentTool {
    pub fn new(specs: Vec<Arc<dyn SubAgentSpec>>) -> Self {
        let names: Vec<&str> = specs.iter().map(|s| s.name()).collect();
        let description_text = format!(
            "Delegate a focused task to a sub-agent. Available sub-agents: {}. \
             Pass `persona` (one of the names) and `prompt` (the task description). \
             The sub-agent runs to completion and returns its final reply.",
            names.join(", ")
        );
        Self {
            specs,
            description_text,
        }
    }
}

impl std::fmt::Debug for SpawnAgentTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnAgentTool")
            .field(
                "specs",
                &self.specs.iter().map(|s| s.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Debug, Deserialize)]
struct SpawnArgs {
    persona: String,
    prompt: String,
}

#[async_trait]
impl Tool for SpawnAgentTool {
    fn name(&self) -> &str {
        "spawn_agent"
    }
    fn description(&self) -> &str {
        &self.description_text
    }
    fn parameters(&self) -> serde_json::Value {
        let names: Vec<String> = self.specs.iter().map(|s| s.name().to_string()).collect();
        serde_json::json!({
            "type": "object",
            "properties": {
                "persona": {"type": "string", "enum": names, "description": "sub-agent name"},
                "prompt": {"type": "string", "description": "task for the sub-agent"}
            },
            "required": ["persona", "prompt"]
        })
    }

    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        if signal.is_aborted() {
            return Err(RuntimeError::Aborted);
        }

        let args: SpawnArgs = serde_json::from_value(invocation.input.clone())
            .map_err(|e| RuntimeError::InvalidInput(format!("spawn_agent args: {e}")))?;

        let spec = self
            .specs
            .iter()
            .find(|s| s.name() == args.persona)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!("unknown sub-agent: {}", args.persona))
            })?;

        let mut child = spec.build()?;
        let outcome = child
            .run(user_text(&args.prompt), signal)
            .await
            .map_err(|e| RuntimeError::ToolFailed(format!("sub-agent failed: {e}")))?;

        // Extract the child's final assistant message as the tool's reply.
        let final_text = child
            .state()
            .messages
            .iter()
            .rev()
            .find_map(|m| match m {
                AgentMessage::Assistant { content, .. } => {
                    let text = content
                        .iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                }
                _ => None,
            })
            .unwrap_or_else(|| "(sub-agent produced no text)".into());

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text { text: final_text }],
            details: Some(serde_json::json!({
                "persona": args.persona,
                "stop_reason": format!("{:?}", outcome.stop_reason),
                "messages_appended": outcome.messages_appended,
            })),
            is_error: false,
        })
    }
}

// Silence unused imports until callers use them.
#[allow(dead_code)]
fn _silence(
    _: SessionId,
    _: MessageId,
    _: ThinkingLevel,
    _: Arc<dyn ModelAdapter>,
    _: Arc<dyn EventSink>,
    _: ToolRegistry,
) {
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
