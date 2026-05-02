//! Bridge: hivecore `AgentEvent` → ACP `SessionUpdate` notifications.
//!
//! The agent loop produces `AgentEvent`s; the ACP server side needs to push
//! `session/update` notifications keyed by `SessionId`. This module owns
//! that translation.

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AgentEvent, Approval, ApprovalAction, ApprovalDecision, ApprovalRequest, ContentBlock,
    EventSink, StopReason,
};

/// Trait the binary's SDK-aware adapter implements; exposed here so
/// `ServerState` can hold it without depending on the SDK directly.
pub trait NotificationSender: Send + Sync + std::fmt::Debug {
    fn send_text_chunk(&self, session_id: &str, text: &str);
    fn send_thought_chunk(&self, session_id: &str, text: &str);
    fn send_tool_call(
        &self,
        session_id: &str,
        tool_call_id: &str,
        name: &str,
        input: &serde_json::Value,
    );
    fn send_tool_call_update(
        &self,
        session_id: &str,
        tool_call_id: &str,
        is_error: bool,
        result: &serde_json::Value,
    );
}

/// Wire vocabulary the bin-side adapter speaks. Mirrors the four
/// `PermissionOptionKind` values from ACP `agent-client-protocol-schema`
/// (`client.rs:671-680`) plus the `Cancelled` outcome (`client.rs:735`).
/// `Approve once` / `Approve always` / `Reject once` / `Reject always` /
/// `Cancelled`. `AcpPrompter` translates these into hivecore's richer
/// `ApprovalDecision` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPermissionDecision {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
    Cancelled,
}

/// Bin-side adapter implements this. Sends a JSON-RPC
/// `session/request_permission` request, awaits the editor's response,
/// returns the decoded outcome.
#[async_trait]
pub trait PermissionRequester: Send + Sync + std::fmt::Debug {
    async fn request_permission(
        &self,
        session_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        raw_input: &serde_json::Value,
        reason: Option<&str>,
    ) -> AcpPermissionDecision;
}

/// `Approval` sink that translates hivecore's HITL trait to ACP
/// `session/request_permission`. Drops in via
/// `ApprovalHook::new(policy, matcher, Arc::new(AcpPrompter::new(...)))`.
///
/// The four standard `PermissionOptionKind` variants from ACP are emitted
/// for every prompt; richer hivecore decisions (`ApprovedAndPersist`)
/// fold into `AllowAlways` for v0.1 (persistent rule writer is v0.2 audit
/// plane).
pub struct AcpPrompter {
    session_id: String,
    requester: Arc<dyn PermissionRequester>,
}

impl AcpPrompter {
    pub fn new(session_id: impl Into<String>, requester: Arc<dyn PermissionRequester>) -> Self {
        Self {
            session_id: session_id.into(),
            requester,
        }
    }
}

impl std::fmt::Debug for AcpPrompter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcpPrompter")
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Approval for AcpPrompter {
    async fn request(&self, req: ApprovalRequest<'_>) -> ApprovalDecision {
        // Surface tool name + raw input for the ToolCallUpdate the editor
        // will render. Hivecore's three action variants map naturally:
        let (tool_call_id, tool_name, raw_input) = match req.action {
            ApprovalAction::Tool {
                invocation_id,
                tool_name,
                input_preview,
                ..
            } => (
                invocation_id.0.clone(),
                tool_name.clone(),
                input_preview.clone(),
            ),
            ApprovalAction::Mcp {
                server,
                tool_name,
                input_preview,
            } => (
                format!("mcp:{server}/{tool_name}"),
                format!("mcp__{server}__{tool_name}"),
                input_preview.clone(),
            ),
            ApprovalAction::ApplyPatch {
                root,
                files,
                changes,
            } => (
                format!("apply_patch:{}", root.display()),
                "apply_patch".to_string(),
                serde_json::json!({
                    "root": root.display().to_string(),
                    "files": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "changes": changes,
                }),
            ),
        };

        let decision = self
            .requester
            .request_permission(
                &self.session_id,
                &tool_call_id,
                &tool_name,
                &raw_input,
                req.reason.as_deref(),
            )
            .await;

        match decision {
            AcpPermissionDecision::AllowOnce => ApprovalDecision::Approved,
            AcpPermissionDecision::AllowAlways => ApprovalDecision::ApprovedForSession,
            AcpPermissionDecision::RejectOnce => ApprovalDecision::Denied,
            // v0.1 has no `AlwaysDeny`; collapse to `Denied`. Goose B5
            // backlog adds the persistent variant.
            AcpPermissionDecision::RejectAlways => ApprovalDecision::Denied,
            AcpPermissionDecision::Cancelled => ApprovalDecision::Cancelled,
        }
    }
}

/// `EventSink` impl that forwards into a `NotificationSender`. The session
/// id is captured at construction time — sinks are per-session.
pub struct AcpEventSink {
    session_id: String,
    sender: Arc<dyn NotificationSender>,
}

impl AcpEventSink {
    pub fn new(session_id: impl Into<String>, sender: Arc<dyn NotificationSender>) -> Self {
        Self {
            session_id: session_id.into(),
            sender,
        }
    }
}

impl std::fmt::Debug for AcpEventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcpEventSink")
            .field("session_id", &self.session_id)
            .finish()
    }
}

#[async_trait]
impl EventSink for AcpEventSink {
    async fn emit(&self, event: AgentEvent) {
        match event {
            AgentEvent::MessageDelta { delta, .. } => match delta {
                ContentBlock::Text { text } => self.sender.send_text_chunk(&self.session_id, &text),
                ContentBlock::Thinking { text } => {
                    self.sender.send_thought_chunk(&self.session_id, &text)
                }
                ContentBlock::ToolUse { id, name, input } => {
                    self.sender
                        .send_tool_call(&self.session_id, &id.0, &name, &input)
                }
                ContentBlock::Image { .. } => {}
            },
            AgentEvent::ToolExecStart {
                tool_call_id,
                name,
                input,
            } => self
                .sender
                .send_tool_call(&self.session_id, &tool_call_id.0, &name, &input),
            AgentEvent::ToolExecEnd {
                tool_call_id,
                is_error,
                result,
            } => self.sender.send_tool_call_update(
                &self.session_id,
                &tool_call_id.0,
                is_error,
                &result,
            ),
            // Streaming-only events the loop still emits but ACP doesn't
            // surface as session/update notifications.
            AgentEvent::AgentStart { .. }
            | AgentEvent::AgentEnd { .. }
            | AgentEvent::TurnStart { .. }
            | AgentEvent::TurnEnd { .. }
            | AgentEvent::MessageStart { .. }
            | AgentEvent::MessageEnd { .. }
            | AgentEvent::MessageCommitted { .. }
            | AgentEvent::ToolExecUpdate { .. }
            | AgentEvent::Custom { .. } => {}
        }
    }
}

/// Map our internal stop reason to the ACP wire vocabulary as a string —
/// the binary's adapter converts it to the SDK's typed enum.
pub fn acp_stop_reason(r: StopReason) -> &'static str {
    match r {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::Refusal => "refusal",
        StopReason::ToolUse => "end_turn", // tool calls already drained inside the loop
        StopReason::StopSequence => "end_turn",
        StopReason::Error => "refusal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hivecore_runtime_core::{
        ApprovalAction, ApprovalDecision, ApprovalRequest, RiskHint, SessionId, ToolCallId, TurnId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct FixedRequester {
        decision: AcpPermissionDecision,
        calls: AtomicUsize,
    }

    impl FixedRequester {
        fn new(d: AcpPermissionDecision) -> Self {
            Self {
                decision: d,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PermissionRequester for FixedRequester {
        async fn request_permission(
            &self,
            _session_id: &str,
            _tool_call_id: &str,
            _tool_name: &str,
            _raw_input: &serde_json::Value,
            _reason: Option<&str>,
        ) -> AcpPermissionDecision {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.decision
        }
    }

    fn tool_action() -> ApprovalAction {
        ApprovalAction::Tool {
            tool_name: "bash".into(),
            invocation_id: ToolCallId("c1".into()),
            input_preview: serde_json::json!({"command": "ls"}),
            risk: RiskHint::default(),
        }
    }

    async fn run(decision: AcpPermissionDecision) -> ApprovalDecision {
        let req_inner = FixedRequester::new(decision);
        let p = AcpPrompter::new("sess-1", Arc::new(req_inner));
        let action = tool_action();
        p.request(ApprovalRequest {
            session_id: SessionId::new(),
            turn_id: TurnId::new(),
            action: &action,
            reason: Some("test".into()),
            explicit_yes_required: false,
        })
        .await
    }

    #[tokio::test]
    async fn allow_once_maps_to_approved() {
        assert!(matches!(
            run(AcpPermissionDecision::AllowOnce).await,
            ApprovalDecision::Approved
        ));
    }

    #[tokio::test]
    async fn allow_always_maps_to_approved_for_session() {
        assert!(matches!(
            run(AcpPermissionDecision::AllowAlways).await,
            ApprovalDecision::ApprovedForSession
        ));
    }

    #[tokio::test]
    async fn reject_once_maps_to_denied() {
        assert!(matches!(
            run(AcpPermissionDecision::RejectOnce).await,
            ApprovalDecision::Denied
        ));
    }

    #[tokio::test]
    async fn reject_always_maps_to_denied_v0_1() {
        // v0.1 collapses persistent reject to one-shot Denied;
        // Goose B5 backlog tracks AlwaysDeny variant.
        assert!(matches!(
            run(AcpPermissionDecision::RejectAlways).await,
            ApprovalDecision::Denied
        ));
    }

    #[tokio::test]
    async fn cancelled_maps_to_cancelled() {
        assert!(matches!(
            run(AcpPermissionDecision::Cancelled).await,
            ApprovalDecision::Cancelled
        ));
    }

    #[tokio::test]
    async fn mcp_action_uses_qualified_name() {
        let req = FixedRequester::new(AcpPermissionDecision::AllowOnce);
        let calls = AtomicUsize::new(0);
        struct Capture {
            tool_name: parking_lot::Mutex<String>,
        }
        #[async_trait]
        impl PermissionRequester for Capture {
            async fn request_permission(
                &self,
                _session_id: &str,
                _tool_call_id: &str,
                tool_name: &str,
                _raw_input: &serde_json::Value,
                _reason: Option<&str>,
            ) -> AcpPermissionDecision {
                *self.tool_name.lock() = tool_name.to_string();
                AcpPermissionDecision::AllowOnce
            }
        }
        impl std::fmt::Debug for Capture {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Capture").finish()
            }
        }
        let cap = Arc::new(Capture {
            tool_name: parking_lot::Mutex::new(String::new()),
        });
        let p = AcpPrompter::new("sess-1", cap.clone());
        let action = ApprovalAction::Mcp {
            server: "git".into(),
            tool_name: "git_status".into(),
            input_preview: serde_json::json!({}),
        };
        let _ = p
            .request(ApprovalRequest {
                session_id: SessionId::new(),
                turn_id: TurnId::new(),
                action: &action,
                reason: None,
                explicit_yes_required: false,
            })
            .await;
        assert_eq!(*cap.tool_name.lock(), "mcp__git__git_status");
        let _ = (req, calls);
    }
}
