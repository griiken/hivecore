use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::*;

/// Trivial hook that records events without blocking — useful for tests
/// in downstream crates.
#[derive(Debug, Default, Clone)]
struct VecLifecycleHook {
    seen: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl LifecycleHook for VecLifecycleHook {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        let tag = match event {
            LifecycleEvent::AgentStart { .. } => "agent_start",
            LifecycleEvent::PreTurn { .. } => "pre_turn",
            LifecycleEvent::PreModelRequest { .. } => "pre_model_request",
            LifecycleEvent::PostMessageCommit { .. } => "post_message_commit",
            LifecycleEvent::PostTurn { .. } => "post_turn",
            LifecycleEvent::AgentEnd { .. } => "agent_end",
        };
        self.seen.lock().unwrap().push(tag);
        LifecycleOutcome::Pass
    }
}

#[tokio::test]
async fn outcome_variants_construct() {
    // Compile-time check that the outcome enum is usable.
    let _: LifecycleOutcome = LifecycleOutcome::Pass;
    let _ = LifecycleOutcome::FailedContinue { reason: "x".into() };
    let _ = LifecycleOutcome::FailedAbort { reason: "y".into() };
    let _ = LifecycleOutcome::ManualAttention { reason: "z".into() };
}

#[tokio::test]
async fn vec_hook_is_object_safe() {
    let h: Arc<dyn LifecycleHook> = Arc::new(VecLifecycleHook::default());
    let res = h
        .on_event(LifecycleEvent::AgentStart {
            session_id: SessionId::new(),
        })
        .await;
    assert!(matches!(res, LifecycleOutcome::Pass));
}
