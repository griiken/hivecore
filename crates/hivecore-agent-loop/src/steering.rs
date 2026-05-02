//! Steering / follow-up sources. Pi exposes three injection points; we keep
//! the same trichotomy because each carries different semantics:
//!
//! - **Steering** — user-typed messages queued mid-flight; injected before
//!   the next assistant turn within an active task.
//! - **Follow-up** — drained after the agent would naturally stop; allows a
//!   subscriber (e.g. a planning extension) to keep the loop alive without
//!   the user typing.

use async_trait::async_trait;
use hivecore_runtime_core::AgentMessage;

#[async_trait]
pub trait SteeringSource: Send + Sync {
    /// Drain any queued user-injected messages.
    async fn drain_steering(&self) -> Vec<AgentMessage> {
        Vec::new()
    }

    /// Drain any queued post-stop follow-up messages.
    async fn drain_follow_ups(&self) -> Vec<AgentMessage> {
        Vec::new()
    }
}

#[derive(Debug, Default, Clone)]
pub struct NoopSteering;

#[async_trait]
impl SteeringSource for NoopSteering {}
