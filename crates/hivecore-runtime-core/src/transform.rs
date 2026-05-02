//! Context transforms. Three pi-style delivery modes (see
//! `.planning/intel/pi-anatomy.md` §"three delivery modes"):
//!   - `transform_outgoing` — rewrite the prompt before each model call.
//!   - `steering_messages`  — appended at the next turn boundary.
//!   - `follow_up_messages` — appended after the current turn ends.

use async_trait::async_trait;

use crate::error::RuntimeResult;
use crate::message::AgentMessage;
use crate::state::AgentState;

#[async_trait]
pub trait ContextTransform: Send + Sync {
    async fn transform_outgoing(
        &self,
        _state: &AgentState,
        messages: Vec<AgentMessage>,
    ) -> RuntimeResult<Vec<AgentMessage>> {
        Ok(messages)
    }

    /// ADR-026 — called by the driver before each model request. If the
    /// transform decides compaction is needed, it returns `Some(marker)`,
    /// where `marker` is an `AgentMessage::Custom { kind: "compaction_marker",
    /// visible_to_model: false, payload: ... }`. The driver appends it to
    /// `state.messages`, fires `PostMessageCommit`, emits `MessageCommitted`,
    /// and only then calls `transform_outgoing` to derive the model payload.
    ///
    /// Default: never compacts.
    async fn maybe_compact(&self, _state: &AgentState) -> RuntimeResult<Option<AgentMessage>> {
        Ok(None)
    }

    async fn steering_messages(&self, _state: &AgentState) -> Vec<AgentMessage> {
        Vec::new()
    }

    async fn follow_up_messages(&self, _state: &AgentState) -> Vec<AgentMessage> {
        Vec::new()
    }
}
