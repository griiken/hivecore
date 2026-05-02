# Examples

Worked examples for the four behavioural rules in [`AGENTS.md`](../AGENTS.md#behavioural-guidelines). Each shows a common LLM failure mode followed by the corrected approach. Code is Rust + hivecore-shaped.

Inspired by [forrestchang/andrej-karpathy-skills `EXAMPLES.md`](https://github.com/forrestchang/andrej-karpathy-skills/blob/main/EXAMPLES.md); examples re-authored for this codebase.

---

## 1. Think before coding

### Hidden assumptions

**Request.** "Add a tool that lets the agent fetch a URL."

**❌ Wrong — silent assumptions**

```rust
pub struct FetchUrlTool;

#[async_trait]
impl Tool for FetchUrlTool {
    async fn execute(&self, inv: ToolInvocation, _abort: AbortSignal, _sink: UpdateSink) -> ToolOutcome {
        let url: String = serde_json::from_value(inv.input["url"].clone()).unwrap();
        let body = reqwest::get(&url).await.unwrap().text().await.unwrap();
        ToolOutcome::ok([ContentBlock::text(body)])
    }
}
```

Assumed: any scheme allowed, no size cap, no tenancy, no audit hook, no allowlist, no abort handling.

**✅ Right — surface assumptions first**

```
Before implementing:

1. Tenancy: per ADR-019 + ADR-028, tools that hit network must carry tenant_id and
   route via tenant-scoped allowlist. Skip or wire?
2. Scheme allowlist: http/https only? data: uris? loopback?
3. Size cap: gsd-browser truncates at 1 KiB tool output. Match or override?
4. Audit: emit on every fetch (ADR-019)? Yes by default.
5. Abort signal: fetch can hang — must respect AbortSignal::cancelled().

Simplest v0.1: HTTP/HTTPS only, 64 KiB cap, tenant allowlist required, abort wired.
Confirm before I write it.
```

### Multiple interpretations

**Request.** "Make the agent loop faster."

**❌ Wrong — pick silently**

Adds prompt cache, swaps to a smaller model, parallelises tool calls, all in one PR.

**✅ Right**

```
"Faster" is ambiguous in our loop:

1. Lower TTFB per turn      → enable Anthropic prompt cache (ADR-XXX)
2. Lower turns-per-task     → tighter system prompt + better tool descriptions
3. Lower wall-time at scale → parallelise independent tool calls (Layer 1 already
                              has the primitive; driver does sequential today)

Each is a separate change with a separate test surface. Which one is biting you?
```

---

## 2. Simplicity first

### Over-abstraction

**Request.** "Add a way to redact secrets from tool output before it hits the audit log."

**❌ Wrong — strategy pattern for one impl**

```rust
pub trait Redactor: Send + Sync { fn redact(&self, s: &str) -> String; }

pub struct RegexRedactor { patterns: Vec<Regex> }
pub struct DenyListRedactor { list: HashSet<String> }
pub struct CompositeRedactor { inner: Vec<Arc<dyn Redactor>> }

pub struct RedactorRegistry { /* ... 80 lines of plumbing ... */ }
```

**✅ Right — one function, grow when a second caller appears**

```rust
pub fn redact_secrets(s: &str, patterns: &[Regex]) -> String {
    patterns.iter().fold(s.to_string(), |acc, re| re.replace_all(&acc, "[REDACTED]").into_owned())
}
```

If a second redaction policy ships → introduce a trait *then*. ADR-016 says Layer 3 is forkable; orgs that need their own policy fork the fn. YAGNI until proven.

---

## 3. Surgical changes

### Don't reformat while you fix

**Request.** "Fix the timeout bug in `wait_selector` — it returns ok when the element never appears."

**❌ Wrong — fixes bug + reformats + adds doc comments + extracts a helper**

```diff
- async fn wait_selector(page: &Page, sel: &str, timeout_ms: u64) -> Result<(), BrowserError> {
-     let deadline = Instant::now() + Duration::from_millis(timeout_ms);
-     while Instant::now() < deadline {
-         if page.find_element(sel).await.is_ok() { return Ok(()) }
-         tokio::time::sleep(Duration::from_millis(100)).await;
-     }
-     Ok(())  // <-- the bug
- }
+ /// Polls `sel` until visible or `timeout_ms` elapses.
+ /// Polling cadence is 100 ms; see ADR-028 for rationale.
+ #[instrument(skip(page))]
+ async fn wait_selector(
+     page: &Page,
+     sel: &str,
+     timeout_ms: u64,
+ ) -> Result<(), BrowserError> {
+     let res = poll_until(timeout_ms, || async {
+         page.find_element(sel).await.is_ok()
+     }).await;
+     res.map_err(|_| BrowserError::WaitTimeout(sel.into()))
+ }
```

Reviewer now has to read every changed line to find the actual fix.

**✅ Right — change the one line that's wrong**

```diff
-     Ok(())
+     Err(BrowserError::WaitTimeout(sel.into()))
```

If `poll_until` is genuinely a better shape, that's a follow-up PR with its own test surface.

### Orphan cleanup

When your edit removes the last caller of `legacy_compute_settle()`, delete it. When you notice an *unrelated* dead `parse_old_format` function, mention it but leave it. Test: every changed line traces to the request.

---

## 4. Goal-driven execution

### Verifiable success, not vibes

**Request.** "There's a session-resume bug — sometimes it loses the last user message."

**❌ Wrong — vague plan, no falsifiable check**

```
I'll review the session resume code and improve the message replay logic.
```

300 lines of "improvements" later, the original bug is still latent and three new ones shipped.

**✅ Right — write the failing test first**

```
Plan:

1. Reproduce: write `tests/resume_loses_last_user_msg.rs` that
   (a) writes 3 turns to a JSONL session,
   (b) ungracefully kills the writer mid-flush of turn 4,
   (c) resumes,
   (d) asserts turn 3's user message is present in derived payload.
   → verify: test fails on main with the bug, passes after the fix.

2. Fix: walk newest-first per ADR-026, treat partial-write as soft-EOF, log a
   warning event.
   → verify: existing 113 tests still green; new test green.

3. Document: append a line in `crates/hivecore-persistence/AGENTS.md` under
   "known crash modes."
   → verify: no other behavioural change.
```

Strong success criteria let you loop without checking in. Weak ones ("make resume robust") force constant clarification.

### Multi-step

For tasks > 3 steps, the plan is the contract. Adapt as facts emerge; do not silently skip a verify.

---

## Quick reference

| Principle | Anti-pattern | Fix |
|---|---|---|
| Think before coding | Silently assumes scheme, scope, tenancy, caps | List assumptions; ask before writing |
| Simplicity first | Trait + 3 impls for the one redactor we have | One function until a second caller exists |
| Surgical changes | Bug fix bundled with reformat + extract + doc | One-line diff; follow-ups are separate PRs |
| Goal-driven | "Review and improve the code" | Failing test → fix → green test → no regressions |

Working if: diffs trace line-by-line to the request, rewrites from overcomplication drop, clarifying questions come before code rather than after a wrong direction.
