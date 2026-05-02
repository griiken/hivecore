use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    Approval, ApprovalDecision, ApprovalRequest, HookOutcome, RiskAugmenter, RiskHint, SessionId,
    ToolCallId, ToolHook, ToolHookContext, ToolInvocation, TurnId,
};

use crate::{ApprovalHook, ApprovalPolicy, ToolNameMatcher};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
struct StaticSink(ApprovalDecision);

#[async_trait]
impl Approval for StaticSink {
    async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
        self.0.clone()
    }
}

fn invocation(name: &str) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId(format!("{name}-1")),
        name: name.into(),
        input: serde_json::json!({"x": 1}),
    }
}

fn ctx<'a>(inv: &'a ToolInvocation) -> ToolHookContext<'a> {
    ToolHookContext {
        session_id: SessionId::new(),
        turn_id: TurnId::new(),
        invocation: inv,
    }
}

#[tokio::test]
async fn unmatched_tool_passes_without_asking() {
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        Arc::new(StaticSink(ApprovalDecision::Denied)),
    );
    let inv = invocation("read_file");
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
}

#[tokio::test]
async fn matched_tool_approved_passes() {
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        Arc::new(StaticSink(ApprovalDecision::Approved)),
    );
    let inv = invocation("bash");
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
}

#[tokio::test]
async fn matched_tool_denied_returns_failed_continue() {
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        Arc::new(StaticSink(ApprovalDecision::Denied)),
    );
    let inv = invocation("bash");
    assert!(matches!(
        hook.before(ctx(&inv)).await,
        HookOutcome::FailedContinue { .. }
    ));
}

#[tokio::test]
async fn abort_returns_manual_attention() {
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        Arc::new(StaticSink(ApprovalDecision::Abort)),
    );
    let inv = invocation("bash");
    assert!(matches!(
        hook.before(ctx(&inv)).await,
        HookOutcome::ManualAttention { .. }
    ));
}

#[tokio::test]
async fn approved_for_session_caches_subsequent_calls() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingSink {
        n: AtomicUsize,
    }
    #[async_trait]
    impl Approval for CountingSink {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.n.fetch_add(1, Ordering::SeqCst);
            ApprovalDecision::ApprovedForSession
        }
    }

    let sink = Arc::new(CountingSink {
        n: AtomicUsize::new(0),
    });
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        sink.clone(),
    );

    let inv = invocation("bash");
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));

    assert_eq!(sink.n.load(Ordering::SeqCst), 1, "sink asked only once");
}

#[tokio::test]
async fn never_policy_rejects_matched_without_asking() {
    #[derive(Debug)]
    struct ShouldNotAsk(AtomicBool);
    #[async_trait]
    impl Approval for ShouldNotAsk {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.0.store(true, Ordering::SeqCst);
            ApprovalDecision::Approved
        }
    }

    let sink = Arc::new(ShouldNotAsk(AtomicBool::new(false)));
    let hook = ApprovalHook::new(
        ApprovalPolicy::Never,
        Arc::new(ToolNameMatcher::coder_defaults()),
        sink.clone(),
    );
    let inv = invocation("bash");
    assert!(matches!(
        hook.before(ctx(&inv)).await,
        HookOutcome::FailedContinue { .. }
    ));
    assert!(
        !sink.0.load(Ordering::SeqCst),
        "Never policy should not call sink"
    );
}

#[tokio::test]
async fn fail_on_ask_policy_rejects_without_invoking_sink() {
    #[derive(Debug)]
    struct ShouldNotAsk(AtomicBool);
    #[async_trait]
    impl Approval for ShouldNotAsk {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.0.store(true, Ordering::SeqCst);
            ApprovalDecision::Approved
        }
    }

    let sink = Arc::new(ShouldNotAsk(AtomicBool::new(false)));
    let hook = ApprovalHook::new(
        ApprovalPolicy::FailOnAsk,
        Arc::new(ToolNameMatcher::coder_defaults()),
        sink.clone(),
    );
    let inv = invocation("bash");
    let outcome = hook.before(ctx(&inv)).await;
    assert!(matches!(outcome, HookOutcome::FailedContinue { .. }));
    if let HookOutcome::FailedContinue { reason } = outcome {
        assert!(reason.contains("fail_on_ask"));
    }
    assert!(
        !sink.0.load(Ordering::SeqCst),
        "FailOnAsk policy must not call sink"
    );
}

#[derive(Debug, Clone, Copy)]
struct StaticRisk(RiskHint);

impl RiskAugmenter for StaticRisk {
    fn augment(&self, _inv: &ToolInvocation) -> RiskHint {
        self.0
    }
}

#[tokio::test]
async fn unless_trusted_auto_allows_read_only_via_augmenter() {
    #[derive(Debug)]
    struct ShouldNotAsk(AtomicBool);
    #[async_trait]
    impl Approval for ShouldNotAsk {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.0.store(true, Ordering::SeqCst);
            ApprovalDecision::Approved
        }
    }

    let sink = Arc::new(ShouldNotAsk(AtomicBool::new(false)));
    let aug: Arc<dyn RiskAugmenter> = Arc::new(StaticRisk(RiskHint {
        read_only: Some(true),
        ..Default::default()
    }));
    let hook = ApprovalHook::new(
        ApprovalPolicy::UnlessTrusted,
        Arc::new(ToolNameMatcher::coder_defaults()),
        sink.clone(),
    )
    .with_augmenter(aug);

    let inv = invocation("bash");
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
    assert!(
        !sink.0.load(Ordering::SeqCst),
        "UnlessTrusted + read_only=Some(true): sink must not be invoked"
    );
}

#[tokio::test]
async fn on_request_does_not_auto_allow_even_with_read_only_hint() {
    use std::sync::atomic::AtomicUsize;
    #[derive(Debug)]
    struct CountingDeny(AtomicUsize);
    #[async_trait]
    impl Approval for CountingDeny {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.0.fetch_add(1, Ordering::SeqCst);
            ApprovalDecision::Denied
        }
    }

    let sink = Arc::new(CountingDeny(AtomicUsize::new(0)));
    let aug: Arc<dyn RiskAugmenter> = Arc::new(StaticRisk(RiskHint {
        read_only: Some(true),
        ..Default::default()
    }));
    let hook = ApprovalHook::new(
        ApprovalPolicy::OnRequest,
        Arc::new(ToolNameMatcher::coder_defaults()),
        sink.clone(),
    )
    .with_augmenter(aug);

    let inv = invocation("bash");
    let _ = hook.before(ctx(&inv)).await;
    assert_eq!(
        sink.0.load(Ordering::SeqCst),
        1,
        "OnRequest must still ask even when read_only=Some(true)"
    );
}

#[tokio::test]
async fn unless_trusted_falls_through_when_augmenter_unset() {
    #[derive(Debug)]
    struct StaticApprove;
    #[async_trait]
    impl Approval for StaticApprove {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            ApprovalDecision::Approved
        }
    }
    let hook = ApprovalHook::new(
        ApprovalPolicy::UnlessTrusted,
        Arc::new(ToolNameMatcher::coder_defaults()),
        Arc::new(StaticApprove),
    );
    let inv = invocation("bash");
    assert!(matches!(hook.before(ctx(&inv)).await, HookOutcome::Pass));
}

#[tokio::test]
async fn hard_deny_matcher_rejects_without_asking() {
    #[derive(Debug)]
    struct ShouldNotAsk(AtomicBool);
    #[async_trait]
    impl Approval for ShouldNotAsk {
        async fn request(&self, _req: ApprovalRequest<'_>) -> ApprovalDecision {
            self.0.store(true, Ordering::SeqCst);
            ApprovalDecision::Approved
        }
    }

    let matcher = ToolNameMatcher::new().deny("rm_rf", "destructive sweep");
    let sink = Arc::new(ShouldNotAsk(AtomicBool::new(false)));
    let hook = ApprovalHook::new(ApprovalPolicy::OnRequest, Arc::new(matcher), sink.clone());
    let inv = invocation("rm_rf");
    let outcome = hook.before(ctx(&inv)).await;
    assert!(matches!(outcome, HookOutcome::FailedContinue { .. }));
    if let HookOutcome::FailedContinue { reason } = outcome {
        assert!(reason.contains("hard-denied"));
    }
    assert!(
        !sink.0.load(Ordering::SeqCst),
        "hard Deny must not invoke the sink"
    );
}
