//! MutationObserver-based settle loop. ADR-028 §"Recommended fixes" v0.2.
//!
//! Adapted from `gsd-build/gsd-browser` `cli/src/daemon/settle.rs`
//! (Apache-2.0 OR MIT, fetched 2026-05-01). Per ADR-024 we vendor the JS
//! snippets + Rust scaffold; renaming window keys to
//! `__hivecoreMutationCounter*` so we never collide with the host page.
//!
//! Replaces the prior `tokio::time::sleep(50ms)` placeholder in `act()`.
//! Concrete failure mode the placeholder had: on slow SPAs the post-action
//! re-snapshot fired during a loading-state mutation; the agent then acted
//! on the half-rendered DOM next turn.
//!
//! Usage:
//! - `ensure_mutation_counter(&page).await` once per navigation. The
//!   observer is persistent for the page lifetime; settle reads deltas.
//! - `settle_after_action(&page, &opts).await` after every CDP-side
//!   action that mutates the DOM.

use std::time::{Duration, Instant};

use chromiumoxide::Page;
use serde::{Deserialize, Serialize};

/// After this many ms with zero observed mutations, shorten the quiet
/// window so totally-idle pages don't pay the full settle delay.
const ZERO_MUTATION_THRESHOLD_MS: u64 = 60;
/// Quiet-window length when zero mutations have been seen.
const ZERO_MUTATION_QUIET_MS: u64 = 30;
/// Defensive timeout per JS call.
const EVALUATE_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettleOptions {
    pub timeout_ms: u64,
    pub poll_ms: u64,
    pub quiet_window_ms: u64,
    pub check_focus_stability: bool,
}

impl Default for SettleOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 1500,
            poll_ms: 50,
            quiet_window_ms: 120,
            check_focus_stability: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettleResult {
    pub settle_ms: u64,
    pub settle_reason: &'static str,
    pub settle_polls: u32,
    pub total_mutations: u64,
    pub url_changed: bool,
}

const INSTALL_OBSERVER_JS: &str = r#"
(() => {
    const key = "__hivecoreMutationCounter";
    const installedKey = "__hivecoreMutationCounterInstalled";
    const w = window;
    if (typeof w[key] !== "number") w[key] = 0;
    if (w[installedKey]) return;
    const observer = new MutationObserver(() => {
        const current = typeof w[key] === "number" ? w[key] : 0;
        w[key] = current + 1;
    });
    observer.observe(document.documentElement || document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
    });
    w[installedKey] = true;
})()
"#;

const READ_STATE_JS_BASE: &str = r##"
((wantFocus) => {
    const w = window;
    const mutationCount = typeof w.__hivecoreMutationCounter === "number" ? w.__hivecoreMutationCounter : 0;
    if (!wantFocus) return JSON.stringify({ mutationCount, focusDescriptor: "" });
    const el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement) {
        return JSON.stringify({ mutationCount, focusDescriptor: "" });
    }
    const id = el.id ? "#" + el.id : "";
    const role = el.getAttribute("role") || "";
    const name = (el.getAttribute("aria-label") || el.getAttribute("name") || "").trim();
    return JSON.stringify({ mutationCount, focusDescriptor: el.tagName.toLowerCase() + id + "|" + role + "|" + name });
})
"##;

#[derive(Debug, Default, Clone)]
struct ReadState {
    mutation_count: u64,
    focus_descriptor: String,
}

/// Idempotent install — safe to call before every action. The persistent
/// observer survives across calls; subsequent calls are no-ops thanks to
/// the `__hivecoreMutationCounterInstalled` flag.
pub async fn ensure_mutation_counter(page: &Page) {
    let _ = tokio::time::timeout(
        EVALUATE_TIMEOUT,
        page.evaluate_expression(INSTALL_OBSERVER_JS),
    )
    .await;
}

async fn read_state(page: &Page, want_focus: bool) -> ReadState {
    let script = format!("({})({})", READ_STATE_JS_BASE.trim(), want_focus);
    let evaluated =
        match tokio::time::timeout(EVALUATE_TIMEOUT, page.evaluate_expression(script.as_str()))
            .await
        {
            Ok(Ok(v)) => v,
            _ => return ReadState::default(),
        };
    let raw: serde_json::Value = match evaluated.into_value() {
        Ok(v) => v,
        Err(_) => return ReadState::default(),
    };
    let parsed: serde_json::Value = match raw {
        serde_json::Value::String(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => return ReadState::default(),
        },
        other => other,
    };
    ReadState {
        mutation_count: parsed
            .get("mutationCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        focus_descriptor: parsed
            .get("focusDescriptor")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

async fn read_url(page: &Page) -> String {
    page.url().await.ok().flatten().unwrap_or_default()
}

/// Returns when the page is "settled" — quiet window without DOM mutations
/// (or URL changes, or focus changes if `check_focus_stability`). Returns
/// the timeout-fallback result on hard timeout; never errors.
pub async fn settle_after_action(page: &Page, opts: &SettleOptions) -> SettleResult {
    let timeout_ms = opts.timeout_ms.max(150);
    let poll_ms = opts.poll_ms.clamp(20, 100);
    let base_quiet_window_ms = opts.quiet_window_ms.max(60);
    let mut active_quiet_window_ms = base_quiet_window_ms;
    let check_focus = opts.check_focus_stability;

    ensure_mutation_counter(page).await;

    let started_at = Instant::now();
    let initial = read_state(page, check_focus).await;
    let mut previous_mutation_count = initial.mutation_count;
    let mut previous_focus = initial.focus_descriptor;
    let mut previous_url = read_url(page).await;

    let mut last_activity_at = Instant::now();
    let mut polls: u32 = 0;
    let mut total_mutations_seen: u64 = 0;
    let mut url_changed = false;

    while started_at.elapsed() < Duration::from_millis(timeout_ms) {
        tokio::time::sleep(Duration::from_millis(poll_ms)).await;
        polls += 1;
        let now = Instant::now();

        let current_url = read_url(page).await;
        if current_url != previous_url {
            url_changed = true;
            previous_url = current_url;
            last_activity_at = now;
        }

        let state = read_state(page, check_focus).await;
        if state.mutation_count > previous_mutation_count {
            total_mutations_seen += state.mutation_count - previous_mutation_count;
            previous_mutation_count = state.mutation_count;
            last_activity_at = now;
        }
        if check_focus && state.focus_descriptor != previous_focus {
            previous_focus = state.focus_descriptor;
            last_activity_at = now;
        }

        // Zero-mutation shortcut: if nothing has happened for
        // ZERO_MUTATION_THRESHOLD_MS, we can declare settled with a much
        // shorter quiet window.
        let elapsed_ms = started_at.elapsed().as_millis() as u64;
        if total_mutations_seen == 0
            && elapsed_ms >= ZERO_MUTATION_THRESHOLD_MS
            && active_quiet_window_ms != ZERO_MUTATION_QUIET_MS
        {
            active_quiet_window_ms = ZERO_MUTATION_QUIET_MS;
        }

        let quiet_elapsed = now.duration_since(last_activity_at).as_millis() as u64;
        if quiet_elapsed >= active_quiet_window_ms {
            let reason = if total_mutations_seen == 0 {
                "zero_mutation_shortcut"
            } else if url_changed {
                "url_changed_then_quiet"
            } else {
                "dom_quiet"
            };
            return SettleResult {
                settle_ms: started_at.elapsed().as_millis() as u64,
                settle_reason: reason,
                settle_polls: polls,
                total_mutations: total_mutations_seen,
                url_changed,
            };
        }
    }

    SettleResult {
        settle_ms: started_at.elapsed().as_millis() as u64,
        settle_reason: "timeout_fallback",
        settle_polls: polls,
        total_mutations: total_mutations_seen,
        url_changed,
    }
}
