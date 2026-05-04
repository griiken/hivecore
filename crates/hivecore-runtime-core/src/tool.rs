//! Tool contract. Object-safe so the registry can store `Arc<dyn Tool>`.
//! Streaming progress flows through `UpdateSink` rather than a return type so
//! tools remain composable with hooks.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::abort::AbortSignal;
use crate::error::RuntimeResult;
use crate::ids::ToolCallId;
use crate::message::ContentBlock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub id: ToolCallId,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub content: Vec<ContentBlock>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
}

impl ToolOutcome {
    pub fn ok(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            details: None,
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: message.into(),
            }],
            details: None,
            is_error: true,
        }
    }
}

/// ADR-034 — controls how tool calls from a single assistant message are
/// dispatched. `Sequential` calls run one-at-a-time, in assistant-message
/// source order; `Parallel` calls run concurrently via `futures::join_all`.
/// Tools that mutate workspace state (`bash`, `write_file`, `edit_file`)
/// override to `Sequential`. Read-only tools (`read_file`, `grep`,
/// `list_dir`) keep the `Parallel` default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionMode {
    Sequential,
    Parallel,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema for arguments.
    fn parameters(&self) -> serde_json::Value;

    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome>;

    /// ADR-034 — hint to the agent loop. Default `Parallel`. Override
    /// `Sequential` for tools that mutate shared state where concurrent
    /// calls would race.
    fn execution_mode(&self) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }

    /// ADR-034 + ADR-029 A3 — per-invocation execution mode. Default
    /// implementation returns `execution_mode()` ignoring the
    /// invocation. Tools whose mode varies per call (e.g. MCP tools
    /// that consult `ToolAnnotations.read_only_hint`) override this
    /// method to dispatch dynamically. The driver calls this method
    /// in `dispatch_tools` so dynamic decisions land transparently.
    fn execution_mode_for(&self, _invocation: &ToolInvocation) -> ToolExecutionMode {
        self.execution_mode()
    }
}

/// Channel for streaming progress updates from a running tool. Cheap to
/// clone; emits are infallible (drops on no listener) by design.
#[derive(Clone)]
pub struct UpdateSink {
    inner: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
}

impl UpdateSink {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(serde_json::Value) + Send + Sync + 'static,
    {
        Self { inner: Arc::new(f) }
    }

    pub fn noop() -> Self {
        Self::new(|_| {})
    }

    pub fn send(&self, v: serde_json::Value) {
        (self.inner)(v);
    }
}

impl std::fmt::Debug for UpdateSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateSink").finish()
    }
}
