//! Tool hooks. Decision space follows Codex's `HookResult`
//! (`Success | FailedContinue | FailedAbort`) extended with
//! `ManualAttention` for the hivecore Gate Plane (ADR-019).

use async_trait::async_trait;

use crate::ids::{SessionId, TurnId};
use crate::tool::{ToolInvocation, ToolOutcome};

#[derive(Debug, Clone)]
pub struct ToolHookContext<'a> {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub invocation: &'a ToolInvocation,
}

#[derive(Debug, Clone)]
pub struct ToolPostContext<'a> {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub invocation: &'a ToolInvocation,
    pub outcome: &'a ToolOutcome,
}

/// Pre-tool decision.
#[derive(Debug, Clone)]
pub enum HookOutcome {
    /// Proceed with normal execution.
    Pass,
    /// Skip execution; return this synthetic outcome instead.
    Override(ToolOutcome),
    /// Soft fail; record as error, let the agent loop continue.
    FailedContinue { reason: String },
    /// Hard fail; abort the current turn.
    FailedAbort { reason: String },
    /// Pause for operator input (Gate Plane manual-attention).
    ManualAttention { reason: String },
}

/// Post-tool mutation hook.
#[derive(Debug, Clone)]
pub enum PostHookOutcome {
    Pass,
    Replace(ToolOutcome),
    /// Same as `Replace`, plus a hint that the agent loop should stop after
    /// the current tool batch settles. Early termination only fires when
    /// EVERY finalized tool result in the batch carries this flag — single
    /// dissenter ⇒ loop continues normally. Preserves "any tool can keep
    /// the conversation alive" semantics. ADR-035.
    ReplaceAndTerminate(ToolOutcome),
}

#[async_trait]
pub trait ToolHook: Send + Sync {
    async fn before(&self, ctx: ToolHookContext<'_>) -> HookOutcome;
    async fn after(&self, ctx: ToolPostContext<'_>) -> PostHookOutcome;
}
