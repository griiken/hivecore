//! Lifecycle hooks. `ToolHook` (in `hook.rs`) gates *tool* calls; this trait
//! gates the rest of the loop: agent start/end, turn boundaries, the
//! pre-model-request moment, and message commits.
//!
//! Why a separate trait: the outcome shape and the call sites are the same
//! (the runtime calls a hook at a checkpoint, gets back a `Pass | Block |
//! Abort | ManualAttention`), but the *event payloads* differ enough that
//! sharing one trait would require an enum-of-enums. Keeping them separate
//! mirrors Codex's split between `hook_runtime` (lifecycle) and the tool
//! permission gate.
//!
//! Compatible with the trichotomy from
//! `.planning/intel/codex-patterns.md` §3.

use async_trait::async_trait;

use crate::ids::{SessionId, TurnId};
use crate::message::AgentMessage;
use crate::message::StopReason;
use crate::model::ModelRequest;

/// One enum, many variants — implementors typically match only on the
/// variants they care about and return `Pass` for the rest.
#[derive(Debug, Clone)]
pub enum LifecycleEvent<'a> {
    /// Fired once when `AgentLoop::run` begins, before the prompt is queued.
    AgentStart { session_id: SessionId },

    /// Fired before each inner-loop iteration (i.e. before the model is
    /// asked to produce a turn). `message_count` is the current depth of
    /// the conversation.
    PreTurn {
        turn_id: TurnId,
        message_count: usize,
    },

    /// Fired right before the runtime calls `ModelAdapter::complete`.
    /// Use this to enforce per-call budgets, rate limits, dry-run mode,
    /// content-policy checks, etc. Return `FailedAbort` to halt the run.
    PreModelRequest { request: &'a ModelRequest },

    /// Fired after a fully-formed message is appended to the session log.
    /// Mirrors `AgentEvent::MessageCommitted` but lets a hook *block* (the
    /// equivalent EventSink path is observer-only).
    PostMessageCommit { message: &'a AgentMessage },

    /// Fired after each turn's stop reason is known.
    PostTurn { turn_id: TurnId, stop: StopReason },

    /// Fired when the run is about to exit (success or abort).
    AgentEnd {
        session_id: SessionId,
        stop: StopReason,
    },
}

/// Hook outcome. Mirrors `HookOutcome` (in `hook.rs`) without the
/// `Override` variant — lifecycle events don't carry a substitutable
/// result.
#[derive(Debug, Clone)]
pub enum LifecycleOutcome {
    Pass,
    FailedContinue { reason: String },
    FailedAbort { reason: String },
    ManualAttention { reason: String },
}

#[async_trait]
pub trait LifecycleHook: Send + Sync {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome;
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
