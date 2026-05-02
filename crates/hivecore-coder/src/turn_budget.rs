//! `LifecycleHook` enforcing a per-run model-call budget.
//!
//! Counts `PreModelRequest` events. When the count exceeds `max_calls`,
//! returns `FailedAbort` to halt the loop. Demonstrates the Gate-plane
//! shape from ADR-019 (typed validators with `pass | hard-fail` outcomes)
//! built atop the substrate without a Layer 3 dependency.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use hivecore_runtime_core::{LifecycleEvent, LifecycleHook, LifecycleOutcome};

#[derive(Debug)]
pub struct TurnBudgetHook {
    max_calls: usize,
    seen: AtomicUsize,
}

impl TurnBudgetHook {
    pub fn new(max_calls: usize) -> Self {
        Self {
            max_calls,
            seen: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl LifecycleHook for TurnBudgetHook {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        if let LifecycleEvent::PreModelRequest { .. } = event {
            let n = self.seen.fetch_add(1, Ordering::Relaxed) + 1;
            if n > self.max_calls {
                return LifecycleOutcome::FailedAbort {
                    reason: format!("turn-budget exceeded ({n} > {})", self.max_calls),
                };
            }
        }
        LifecycleOutcome::Pass
    }
}
