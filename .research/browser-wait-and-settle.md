# WaitCondition + Settle — implementation research

Sources fetched 2026-05-01:
- gsd-build/gsd-browser @ main — `cli/src/daemon/handlers/wait.rs` (296 lines), `cli/src/daemon/settle.rs` (247 lines). Apache-2.0 + MIT.
- microsoft/playwright-mcp @ main — README only; no separate wait tool source (reliance on Playwright auto-wait).
- mattsse/chromiumoxide @ main — `src/page.rs` line 351–356 (only `wait_for_navigation*` exist).

## WaitCondition (Concern B)

### gsd-browser implementation

Constants (`wait.rs:14-15`):
```
DEFAULT_TIMEOUT_MS = 10_000
POLL_INTERVAL_MS   = 100
```

Single generic helper `poll_until(deadline, interval, check)` runs `check().await`, sleeps on miss, returns false on deadline (`wait.rs:210-225`). All polled variants reuse it. Eval is **Rust-side** — JS work is delegated to `inspection::*` queries the daemon already exposes. Timeout returns `met:false` (NOT an error) — the caller decides; gsd treats as "fail-success" semantics.

#### selector_visible / selector_hidden (`wait.rs:54-81`)
```rust
poll_until(deadline, poll_interval, || async {
    inspection::selector_query(page, state, value, true).await
        .ok()
        .and_then(|r| r.get("first").cloned())
        .and_then(|f| f.get("visible").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}).await
```
- Polled in **Rust** at 100 ms cadence.
- "Visible" comes from the daemon's `inspection::selector_query` (one CDP roundtrip per poll).
- `selector_hidden` succeeds when `count == 0 || !visible`.

#### url_contains (`wait.rs:82-97`)
- Reads `inspection::target_url(page, state)` per poll, calls `String::contains(value)`.
- **Substring match in Rust**, no regex.

#### url_matches
- **NOT IMPLEMENTED in gsd-browser**. Only substring `url_contains` exists. They never built regex URL matching.

#### text_visible / text_hidden (`wait.rs:98-117`)
- `inspection::text_query` polls page for the substring; returns `{found:bool}`.
- `text_hidden` is just the negation.

#### network_idle (`wait.rs:118-137`)
- **Daemon-side** — uses an in-process `logs.network` ring buffer (NOT CDP `Network.requestWillBeSent` counters in JS).
- Definition: **500 ms window with no new entries appended**.
- Resets `stable_since` when `logs.network.len()` increments.
- No "max 2 in-flight" rule — pure quiescence-of-the-log.
- Poll cadence 100 ms; deadline checked between polls.

#### request_completed (`wait.rs:138-144`)
Bonus variant: any URL substring match in `logs.network.snapshot()`. Useful pattern for hivecore's `NetworkIdle` family.

#### console_message, element_count, region_stable
- `element_count` uses parsed thresholds (`>=3`, `==0`, `<5`). Worth borrowing.
- `region_stable` — innerHTML hash, requires 2 consecutive identical hashes (`wait.rs:165-191`). Different mechanic than mutation observer.

#### ref_visible
- **NOT IMPLEMENTED as a wait condition in gsd-browser**. Refs (`@v1:e1`) exist for clicks, but `wait-for` doesn't enumerate a `ref_visible` variant. Hivecore must build it: resolve ref → CSS selector via the same registry hivecore uses for `click_ref`, then reuse `selector_visible` polling.

#### Load / DomContentLoaded
- gsd-browser does not expose these as `wait-for` conditions. Navigation handlers (`navigate.rs`) take a `wait_until: load|domcontentloaded|networkidle` argument and consume it before returning. Same in playwright-mcp's `browser_navigate`.
- Recommendation for hivecore: keep `Load` / `DomContentLoaded` as wait conditions but back them with chromiumoxide's `Page::wait_for_navigation()` (file `src/page.rs:356`) plus a CDP `Page.lifecycleEvent` subscription.

### chromiumoxide native helpers

From `src/page.rs`:
```
pub async fn wait_for_navigation_response(&self) -> Result<ArcHttpRequest>   // line 351
pub async fn wait_for_navigation(&self) -> Result<&Self>                     // line 356
pub async fn find_element(&self, selector) -> Result<Element>                // line 497
pub async fn find_elements(&self, selector) -> Result<Vec<Element>>          // line 504
```

There is **no** `wait_for_selector`, **no** `wait_for_function`, **no** `wait_for_load_state`. Settle-helpers do not exist. We must build all of them on top of `evaluate_expression` + a polling loop. `find_element` raises an error when missing — usable as a one-shot existence check inside `poll_until`, but it has no built-in timeout.

### playwright-mcp comparison

Vocabulary (README, `browser_wait_for`):
- `time` (seconds) → our `Delay`.
- `text` → our `TextVisible`.
- `textGone` → our `TextHidden`.

That's the entire MCP-exposed surface. Everything else is **Playwright auto-wait**: `click`, `fill`, etc. internally wait for the locator to be `visible + stable + enabled + receives-events` before acting. The tool layer doesn't expose `selector_visible` etc. because Playwright actions wait themselves.

Trade-off: Playwright's philosophy means agents rarely need explicit waits; the cost is that Playwright is a heavyweight runtime hivecore can't embed (Node + browsers package). gsd-browser's explicit-wait surface is the right shape for a chromiumoxide-based harness.

### Recommendation for hivecore

| Variant | Eval location | Cadence | Default timeout | On timeout |
|---|---|---|---|---|
| `Delay(ms)` | Rust `tokio::time::sleep` | n/a | n/a | always succeeds |
| `Load` | chromiumoxide `wait_for_navigation()` + CDP `Page.lifecycleEvent` (`name=="load"`) | event-driven | 30s | error |
| `DomContentLoaded` | CDP `Page.lifecycleEvent` (`name=="DOMContentLoaded"`) | event-driven | 30s | error |
| `SelectorVisible(sel)` | Rust poll → JS `evaluate_expression` (`getBoundingClientRect + checkVisibility`) | 100 ms | 10s | `met:false` |
| `SelectorHidden(sel)` | same, negated; succeed on `count==0 \|\| !visible` | 100 ms | 10s | `met:false` |
| `UrlContains(s)` | Rust poll → `page.url()` → `String::contains` | 100 ms | 10s | `met:false` |
| `UrlMatches(re)` | **compile regex in Rust once** (`regex::Regex`), poll `page.url()`, `re.is_match(&url)` | 100 ms | 10s | `met:false` |
| `TextVisible(s)` | Rust poll → JS that walks DOM with `TreeWalker(NodeFilter.SHOW_TEXT)` and returns `{found: bool}` (or use `document.body.innerText.includes(...)` for v0.1) | 100 ms | 10s | `met:false` |
| `TextHidden(s)` | same, negated | 100 ms | 10s | `met:false` |
| `RefVisible(ref)` | resolve ref → selector via hivecore ref registry, then `SelectorVisible` impl | 100 ms | 10s | `met:false` |
| `NetworkIdle{idle_ms,timeout}` | Rust poll → daemon's network-event ring buffer; require `idle_ms` (default 500) without new entries | 100 ms | 10s | `met:false` |

Keep `met: bool` semantics in the response (gsd does this). Promotion to error is the caller's choice. URL regex MUST be compiled in Rust — JS `RegExp` is fine but Rust `regex` is stricter, error-on-compile, and centralizes the syntax surface. Reuse one `poll_until` helper; do not inline the loop per variant.

## Settle loop (Concern C)

### gsd-browser full algorithm (verbatim)

`settle.rs:25-42` — **MutationObserver install JS**:
```js
(() => {
    const key = "__piMutationCounter";
    const installedKey = "__piMutationCounterInstalled";
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
```

`settle.rs:44-56` — **Read state JS** (called every poll):
```js
((wantFocus) => {
    const w = window;
    const mutationCount = typeof w.__piMutationCounter === "number" ? w.__piMutationCounter : 0;
    if (!wantFocus) return { mutationCount, focusDescriptor: "" };
    const el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement) {
        return { mutationCount, focusDescriptor: "" };
    }
    const id = el.id ? `#${el.id}` : "";
    const role = el.getAttribute("role") || "";
    const name = (el.getAttribute("aria-label") || el.getAttribute("name") || "").trim();
    return { mutationCount, focusDescriptor: `${el.tagName.toLowerCase()}${id}|${role}|${name}` };
})
```

`settle.rs:127-221` — **Rust glue (key excerpts)**:

```rust
let timeout_ms = opts.timeout_ms.max(150);
let poll_ms = opts.poll_ms.clamp(20, 100);
let base_quiet_window_ms = opts.quiet_window_ms.max(60);

ensure_mutation_counter(page).await;             // idempotent install
let initial = read_settle_state(page, check_focus).await;
let mut previous_mutation_count = initial.mutation_count;
let mut previous_focus = initial.focus_descriptor;
let mut previous_url = read_page_url(page).await;

while started_at.elapsed().as_millis() < timeout_ms as u128 {
    sleep(Duration::from_millis(poll_ms)).await;
    polls += 1;
    let now = Instant::now();

    let current_url = read_page_url(page).await;
    if current_url != previous_url { saw_url_change = true; previous_url = current_url; last_activity_at = now; }

    let state = read_settle_state(page, check_focus).await;
    if state.mutation_count > previous_mutation_count {
        total_mutations_seen += state.mutation_count - previous_mutation_count;
        previous_mutation_count = state.mutation_count;
        last_activity_at = now;
    }
    if check_focus && state.focus_descriptor != previous_focus {
        previous_focus = state.focus_descriptor;
        last_activity_at = now;
    }

    // Zero-mutation shortcut
    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    if total_mutations_seen == 0 && elapsed_ms >= ZERO_MUTATION_THRESHOLD_MS
        && active_quiet_window_ms != ZERO_MUTATION_QUIET_MS {
        active_quiet_window_ms = ZERO_MUTATION_QUIET_MS;       // 30 ms
    }

    let quiet_elapsed = now.duration_since(last_activity_at).as_millis() as u64;
    if quiet_elapsed >= active_quiet_window_ms {
        // settled — return reason ∈ {zero_mutation_shortcut, dom_quiet, url_changed_then_quiet}
        return SettleResult { ... };
    }
}
// timeout_fallback
```

### Quiet-window + hard timeout values

Constants in `settle.rs:14-21`:
- `ZERO_MUTATION_THRESHOLD_MS = 60` — after 60 ms of zero observed mutations, shorten the quiet window.
- `ZERO_MUTATION_QUIET_MS = 30` — shortened quiet window when nothing has happened.
- Base `quiet_window_ms` clamped to `>= 60` (caller-supplied; library doesn't ship a hard default but call-sites use ~120 ms).
- Hard `timeout_ms` clamped to `>= 150`; `poll_ms` clamped to `[20, 100]`.
- `EVALUATE_TIMEOUT = 25 s` per `page.evaluate_expression` call (defensive against a hung renderer).
- `PAGE_URL_TIMEOUT = 2 s` per `page.url()` call.

### Mutation counter mechanic

- **Global window counter** at `window.__piMutationCounter`. Single integer.
- Observer is **persistent** — `installedKey` flag prevents re-install on subsequent settle calls. Observer keeps running for the page lifetime; settle calls just read deltas (`state.mutation_count - previous_mutation_count`).
- This is critical: re-installing the observer on each call would miss mutations that happen between calls.
- Observer scope: `document.documentElement || document.body` with `subtree + childList + attributes + characterData`. No filter on attribute names (CSS animations *can* trigger churn — see edge cases).

### Edge cases handled

- **Long-poll requests**: settle does not look at network at all — pure DOM/URL/focus signal. A hung XHR keeps not mutating the DOM, so it doesn't block settle. (Trade-off: if the hung XHR holds back a render, you wait for the eventual render's mutations.)
- **CSS animations / transitions firing mutations**: `attributes: true` *does* catch `style`/`class` flips. They have no filter. Mitigation: the zero-mutation shortcut reduces wait when truly idle, but constant CSS churn can stretch settle to `timeout_fallback`. Not perfectly handled.
- **Long-poll-style heartbeats** (timer ticks updating a clock element): same as animations — will hold settle until `timeout_fallback`. Document this as a known quirk.
- **Multiple consecutive actions**: observer is reused (`installedKey`). Each settle reads a delta from `previous_mutation_count`. Cheap and correct.
- **URL change**: tracked alongside mutations; resets `last_activity_at`. Reason becomes `url_changed_then_quiet`.
- **Focus stability** (`check_focus_stability`): optional. Useful for forms — page might be DOM-quiet but the focus is still bouncing as React commits.

### chromiumoxide / playwright comparison

- **chromiumoxide**: zero settle helpers. We're on our own. Confirmed by reading `src/page.rs`.
- **Playwright**: settle is implicit and per-action, not exposed. Each `locator.click()` waits for `visible + stable + enabled + receives events + not detached`. Their "stability" check is their settle equivalent: it polls `getBoundingClientRect()` for two consecutive raf-aligned frames with identical box. Different signal (geometry, not mutations), narrower scope (per-element not page). Better for click correctness, worse for "is the SPA done re-rendering?" — gsd's mutation-observer model is the right one for hivecore's snapshot-then-act loop.

### Recommendation for hivecore

1. New file `crates/hivecore-browser-runtime/src/settle.rs` (or wherever the chromiumoxide impl lives). Vendor the JS strings verbatim from gsd-browser; rename the window keys to `__hivecoreMutationCounter` / `__hivecoreMutationCounterInstalled` so we never collide with a host page.
2. Public API:
   ```rust
   pub struct SettleOptions { timeout_ms: u64, poll_ms: u64, quiet_window_ms: u64, check_focus_stability: bool }
   pub struct SettleResult { settle_ms: u64, settle_reason: &'static str, settle_polls: u32 }
   pub async fn ensure_mutation_counter(page: &Page);
   pub async fn settle_after_action(page: &Page, opts: &SettleOptions) -> SettleResult;
   ```
3. Defaults: `timeout_ms: 1500`, `poll_ms: 50`, `quiet_window_ms: 120`, `check_focus_stability: false`. Same clamps as gsd (`timeout >= 150`, `poll ∈ [20,100]`, `quiet >= 60`).
4. Constants (vendor verbatim): `ZERO_MUTATION_THRESHOLD_MS = 60`, `ZERO_MUTATION_QUIET_MS = 30`.
5. Replace the existing `tokio::time::sleep(Duration::from_millis(50))` call site in the `act()` flow with `settle_after_action(page, &SettleOptions::default()).await`.
6. Call `ensure_mutation_counter` once on every page navigation (CDP `Page.frameNavigated` listener) so the counter survives full page loads. Single source: a hook on the navigation handler.
7. Plumb `SettleResult` into the audit event so we can spot pages that always hit `timeout_fallback` (broken animations, ticking clocks).

## License compatibility

gsd-browser is **Apache-2.0 OR MIT** — same dual-license posture as hivecore. Per ADR-024 (vendor upstream verbatim), copy the JS snippets and the Rust scaffold into hivecore with attribution:
- File comment: `// Adapted from gsd-build/gsd-browser cli/src/daemon/settle.rs (Apache-2.0 OR MIT, fetched 2026-05-01).`
- Add a row to `crates/<crate>/THIRD_PARTY.md` (or extend the prompts/README pattern from ADR-024) noting source URL + commit + license.
- No CLA required. No per-file LICENSE header needed (root LICENSE files cover us).

## Recommended implementation order

1. **Settle loop first** — biggest correctness gain, smallest surface, no protocol design. Replace placeholder sleep, add audit field.
2. **Refactor `WaitCondition` dispatch around a `poll_until` helper.** Keep the existing enum; route working variants (`Delay`/`Load`/`DomContentLoaded`) untouched, point each missing variant at its real evaluator. Drop the silent-fall-through arm — make unimplemented variants explicit `Err("not yet implemented: <variant>")` until each is filled.
3. **Implement `SelectorVisible` / `SelectorHidden`** — share the JS evaluator with the future snapshot pipeline (`getBoundingClientRect + checkVisibility`).
4. **Implement `UrlContains` / `UrlMatches`** — Rust-side `String::contains` and `regex::Regex::is_match` against `page.url()`. Compile regex once at construction; surface compile errors to the caller.
5. **Implement `TextVisible` / `TextHidden`** — single JS that walks `document.body.innerText` (good enough for v0.1; promote to TreeWalker if false negatives bite).
6. **Implement `NetworkIdle`** — depends on a network-event ring buffer in the daemon. If that doesn't exist yet, gate behind a feature flag and use CDP `Network.requestWillBeSent` / `Network.loadingFinished` counters via JS as v0.1 fallback.
7. **Implement `RefVisible`** — needs the ref-resolution registry. After it lands, becomes a one-line shim over `SelectorVisible`.
8. **Promote `Load` / `DomContentLoaded` to event-driven** — replace any current polling with CDP `Page.lifecycleEvent` subscription via chromiumoxide's event stream.
