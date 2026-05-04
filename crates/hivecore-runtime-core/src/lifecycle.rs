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

use crate::ids::{MessageId, SessionId, TurnId};
use crate::message::AgentMessage;
use crate::message::StopReason;
use crate::model::ModelRequest;
use crate::state::AgentState;

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

    /// ADR-036 — fired when `ContextTransform::maybe_compact` produced a
    /// marker but BEFORE it is appended to `state.messages`. A hook
    /// returning `FailedContinue` cancels this round of compaction (the
    /// marker is dropped, no event is emitted, the loop continues with the
    /// original messages). `FailedAbort` / `ManualAttention` bubble as
    /// usual via `LoopError`. Hooks read the unappended marker payload
    /// (trigger / tokens / file_refs) directly off `marker` — see
    /// `crates/hivecore-compaction/src/marker.rs::CompactionMarker`.
    PreCompact {
        state: &'a AgentState,
        marker: &'a AgentMessage,
    },

    /// ADR-036 — fired after the compaction marker is appended to the
    /// session log. `marker_id` matches the `id()` of the appended message;
    /// downstream consumers (audit plane, KG ingestion, frontend progress
    /// indicators) can correlate against the on-disk `MessageCommitted`
    /// stream. Non-`Pass` outcomes here are recorded but do not unwind the
    /// append (compaction is already on disk).
    PostCompact {
        state: &'a AgentState,
        marker_id: &'a MessageId,
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
