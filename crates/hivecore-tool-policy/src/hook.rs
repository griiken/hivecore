//! `ApprovalHook` — `ToolHook` impl that brokers an `Approval` round-trip.

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    Approval, ApprovalAction, ApprovalDecision, ApprovalRequest, HookOutcome, PostHookOutcome,
    RiskAugmenter, ToolHook, ToolHookContext, ToolPostContext,
};

use crate::cache::SessionCache;
use crate::matcher::{MatchOutcome, ToolMatcher};
use crate::policy::ApprovalPolicy;

#[derive(Debug)]
pub struct ApprovalHook {
    policy: ApprovalPolicy,
    matcher: Arc<dyn ToolMatcher>,
    sink: Arc<dyn Approval>,
    cache: Arc<SessionCache>,
    augmenter: Option<Arc<dyn RiskAugmenter>>,
}

impl ApprovalHook {
    pub fn new(
        policy: ApprovalPolicy,
        matcher: Arc<dyn ToolMatcher>,
        sink: Arc<dyn Approval>,
    ) -> Self {
        Self {
            policy,
            matcher,
            sink,
            cache: Arc::new(SessionCache::new()),
            augmenter: None,
        }
    }

    pub fn with_cache(mut self, cache: Arc<SessionCache>) -> Self {
        self.cache = cache;
        self
    }

    /// Install a `RiskAugmenter` (e.g. `McpRiskAugmenter` consuming MCP
    /// `read_only_hint`). When set, `policy = UnlessTrusted` auto-allows
    /// invocations whose augmented `RiskHint.read_only == Some(true)`.
    pub fn with_augmenter(mut self, augmenter: Arc<dyn RiskAugmenter>) -> Self {
        self.augmenter = Some(augmenter);
        self
    }
}

#[async_trait]
impl ToolHook for ApprovalHook {
    async fn before(&self, ctx: ToolHookContext<'_>) -> HookOutcome {
        let inv = ctx.invocation;

        // 1. Matcher classification — 3-state outcome.
        let reason = match self.matcher.classify(inv) {
            MatchOutcome::Allow => return HookOutcome::Pass,
            MatchOutcome::Deny(reason) => {
                return HookOutcome::FailedContinue {
                    reason: format!("{} hard-denied: {reason}", inv.name),
                };
            }
            MatchOutcome::Ask(reason) => reason,
        };

        // 2. Policy short-circuits before invoking the sink.
        match self.policy {
            ApprovalPolicy::Never => {
                return HookOutcome::FailedContinue {
                    reason: format!("{} blocked by policy `never`: {reason}", inv.name),
                };
            }
            ApprovalPolicy::FailOnAsk => {
                return HookOutcome::FailedContinue {
                    reason: format!(
                        "{} requires approval but policy is `fail_on_ask` (headless): {reason}",
                        inv.name
                    ),
                };
            }
            ApprovalPolicy::UnlessTrusted | ApprovalPolicy::OnRequest => { /* fall through */ }
        }

        // 3. Risk augmentation — pull RiskHint from the augmenter (e.g.
        //    `McpRiskAugmenter` reads MCP `read_only_hint`). Synchronous
        //    cache lookup; defaults to empty if no augmenter installed.
        let risk = self
            .augmenter
            .as_ref()
            .map(|a| a.augment(inv))
            .unwrap_or_default();

        // 3a. UnlessTrusted short-circuit — confirmed read-only auto-allows.
        //     Spec MUST: annotations are untrusted unless server is trusted.
        //     The augmenter is responsible for honoring per-server trust;
        //     here we trust whatever the augmenter returns.
        if matches!(self.policy, ApprovalPolicy::UnlessTrusted) && risk.read_only == Some(true) {
            tracing::debug!(tool = %inv.name, "auto-allow: read_only confirmed");
            return HookOutcome::Pass;
        }

        // 4. Session cache hit — previously approved-for-session.
        if self.cache.contains(&inv.name, &inv.input) {
            tracing::debug!(tool = %inv.name, "approval cache hit");
            return HookOutcome::Pass;
        }

        // 5. Ask the human.
        let action = ApprovalAction::Tool {
            tool_name: inv.name.clone(),
            invocation_id: inv.id.clone(),
            input_preview: inv.input.clone(),
            risk,
        };
        let req = ApprovalRequest {
            session_id: ctx.session_id,
            turn_id: ctx.turn_id,
            action: &action,
            reason: Some(reason.clone()),
            explicit_yes_required: false,
        };
        let decision = self.sink.request(req).await;

        // 6. Map decision → HookOutcome.
        match decision {
            ApprovalDecision::Approved | ApprovalDecision::ApprovedAndPersist { .. } => {
                HookOutcome::Pass
            }
            ApprovalDecision::ApprovedForSession => {
                self.cache.insert(&inv.name, &inv.input);
                HookOutcome::Pass
            }
            ApprovalDecision::Denied | ApprovalDecision::TimedOut => HookOutcome::FailedContinue {
                reason: format!("user denied {}: {reason}", inv.name),
            },
            ApprovalDecision::Abort | ApprovalDecision::Cancelled => HookOutcome::ManualAttention {
                reason: format!("user aborted at {}: {reason}", inv.name),
            },
        }
    }

    async fn after(&self, _ctx: ToolPostContext<'_>) -> PostHookOutcome {
        PostHookOutcome::Pass
    }
}
