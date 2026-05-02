use std::sync::Arc;

use async_trait::async_trait;

use super::*;

/// Always-approve sink — exists to prove the trait is object-safe.
#[derive(Debug, Default)]
struct AutoApprove;

#[async_trait]
impl Approval for AutoApprove {
    async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
        ApprovalDecision::Approved
    }
}

#[tokio::test]
async fn approval_trait_is_object_safe() {
    let sink: Arc<dyn Approval> = Arc::new(AutoApprove);
    let action = ApprovalAction::Tool {
        tool_name: "bash".into(),
        invocation_id: ToolCallId("call-1".into()),
        input_preview: serde_json::json!({"command": "ls"}),
        risk: RiskHint {
            read_only: Some(true),
            ..Default::default()
        },
    };
    let req = ApprovalRequest {
        session_id: SessionId::new(),
        turn_id: TurnId::new(),
        action: &action,
        reason: None,
        explicit_yes_required: false,
    };
    assert!(matches!(
        sink.request(req).await,
        ApprovalDecision::Approved
    ));
}

#[test]
fn action_round_trips_json() {
    let action = ApprovalAction::Mcp {
        server: "everything".into(),
        tool_name: "echo".into(),
        input_preview: serde_json::json!({"message": "hi"}),
    };
    let s = serde_json::to_string(&action).unwrap();
    assert!(s.contains("\"kind\":\"mcp\""));
    let back: ApprovalAction = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, ApprovalAction::Mcp { .. }));
}

#[test]
fn decision_round_trips_with_amendment() {
    let d = ApprovalDecision::ApprovedAndPersist {
        rule: ApprovalRule::McpToolAllow {
            server: "everything".into(),
            tool: "echo".into(),
        },
    };
    let s = serde_json::to_string(&d).unwrap();
    assert!(s.contains("\"decision\":\"approved_and_persist\""));
    let back: ApprovalDecision = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, ApprovalDecision::ApprovedAndPersist { .. }));
}

#[test]
fn scope_default_is_call() {
    assert_eq!(ApprovalScope::default(), ApprovalScope::Call);
}

#[test]
fn risk_hint_defaults_unknown() {
    let r = RiskHint::default();
    assert!(r.read_only.is_none());
    assert!(r.risky.is_none());
    assert!(r.network.is_none());
}
