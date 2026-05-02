# crates/hivecore-browser-harness/AGENTS.md

**Status: v0.1.1 IMPLEMENTED + validated against OSS prior art.**

Design contract from ADR-028 realised across three crates. Two rounds of
validation against gsd-build/gsd-browser, microsoft/playwright-mcp,
1jehuang/jcode, and the chromiumoxide upstream issue tracker (#292, #320).
Reports at `.research/browser-harness-validation.md` (round 1) and
`.research/browser-harness-postfix-validation.md` (round 2 sign-off).

End-to-end via `cargo run --example {smoke,act} -p hivecore-browser-harness`
passes on Linux with `/usr/bin/google-chrome-stable` installed. Real CDP
dispatch (no stubs) for click / fill / type / press / hover / set_checked /
select / scroll. React-controlled inputs work (verified — gsd-browser's
prototype-setter pattern + `input`/`change` event dispatch).

Stale-ref guard correct: missing id → `StaleRef`, version mismatch →
`StaleRef`, disabled element → `NotActionable("(disabled in current
snapshot)")`. Type-level enforcement of re-snapshot-after-action invariant.

v0.2 LANDED (2026-05-02):
- **MutationObserver settle loop** replaced the 50 ms placeholder
  (`crates/hivecore-browser-runtime/src/settle.rs`, vendored from
  gsd-browser).
- **`WaitCondition` predicates real**: all 11 variants
  (`selector_visible/hidden`, `ref_visible`, `url_contains`,
  `url_matches` regex, `text_visible/hidden`, `network_idle`,
  `delay`, `load`, `dom_content_loaded`).
- `wait` example covers positive + negative timing.

v0.2.x backlog (still deferred):
- Per-frame routing (iframes) via CDP `Target.attachToTarget`.
- Vision-fallback tools (`browser_scene_understand`/`browser_vision_click`).
- `accessibleName` `aria-labelledby` chained-id resolver beyond first hit.
- `text_visible` assertion that searches `document.body.innerText`, not
  just element accessible names.
- `NetworkIdle` via CDP `Network.requestWillBeSent` ring buffer
  (current JS-side `PerformanceObserver` misses WebSocket/SSE traffic).
- `SelectorVisible` / `TextVisible` shadow-DOM piercing (current impl
  walks light-DOM only; the snapshot pipeline already handles shadow,
  these waits should too).
- `TextVisible` case-insensitive option (currently exact substring).
- Real CDP `Page.lifecycleEvent` subscription for `Load` /
  `DomContentLoaded` (currently both go through `wait_for_navigation()`).
- 5th settle reason `evaluate_error` for the case where
  `evaluate_expression` itself fails — currently silently absorbed.

Linked artifacts:
- `.planning/intel/browser-harness.md` — distilled prior-art research (the
  source-of-truth for design decisions in this crate).
- `.research/browser-tools.md` — long-form per-source survey.
- ADR-028 (planned) — browser harness architecture.

---

## Layer

**Layer 3.** I/O-heavy: spawns Chrome subprocess, owns CDP websocket, manages
artifact directory, persists per-tenant credential vault. Must not be
introduced anywhere upstream of Layer 2 (`hivecore-agent-loop`,
`hivecore-runtime-core`).

## Crate map (planned)

This crate is one of three coordinated crates. Implement in this order:

| Crate | Layer | Responsibility | When |
|---|---|---|---|
| `hivecore-browser-core` | 2 | `BrowserProvider` trait, capability schema, snapshot/ref/action types. Pure, no I/O. Lifted from jcode's BPP (MIT, attribution required). | First |
| `hivecore-browser-runtime` | 3 | Default native provider — `chromiumoxide` daemon, persistent CDP, ref versioning, artifact dir, credential vault. Patterns vendored from gsd-browser (Apache-2.0). | Second |
| `hivecore-browser-harness` (this crate) | 3 | The ~20 typed `Tool` impls. Consumes `Arc<dyn BrowserProvider>`. Default-wired in `hivecore-coder` and `hivecore-acp-server` with `--no-browser` opt-out. | Third |

The "harness" crate is intentionally thin — it is the `Tool` adapter layer
over the provider trait. All real I/O lives in `hivecore-browser-runtime`.

## Tool surface (~20 typed tools)

Names align with Playwright MCP convention so MCP-trained models recognise
them. **`browser_act` is one tool with a typed `kind` enum**, not a separate
tool per click/type/hover — keeps the model's tool-list compact.

```
browser_session       — open / close / list / switch (carries tenant_id)
browser_navigate      — { url, wait_until }
browser_back / browser_forward / browser_reload
browser_snapshot      — a11y tree + actionable elements + versioned refs
browser_act           — { ref, kind: Click | Type | Hover | Press | Select
                                | SetChecked | Fill | Drag | Upload | Scroll }
browser_wait          — { condition: Load | DomContentLoaded | NetworkIdle
                                    | Selector | Ref | UrlContains | TextVisible }
browser_assert        — { ref, predicate: TextEquals | AttrEquals | Visible
                                          | Count | UrlMatches }
browser_screenshot    — page or region (artifact path; not in model prompt)
browser_extract       — structured extraction (selector → fields)
browser_network       — list / route / block / get-request
browser_console       — list messages
browser_eval          — capability-gated; default off
browser_trace         — start / stop / export

# Vision fallback (delegated, slower path) — TesterArmy hybrid
browser_scene_understand   — capture screenshot, return compact scene
                              summary, flag a11y/visual mismatches
browser_vision_click       — { element: <intent text> } → vision model
                              localises target → click → fresh snapshot
```

## Hard invariants

1. **A11y snapshot is the default observation channel.** Every action must
   reference an opaque versioned ref returned from the latest
   `browser_snapshot`. Coordinate-based actions are vision-fallback only.
2. **Refs are versioned and stale on navigation.** After any DOM-changing
   action, the next `browser_snapshot` invalidates older refs. Tools that
   accept stale refs must return a typed `StaleRef` error, never silently
   resolve.
3. **Coordinates never leave the browser-runtime crate.** They live inside
   `browser_vision_click`'s implementation and are recorded in
   `ToolOutcome.details` for audit. Primary agent transcript stays
   coordinate-free.
4. **`tenant_id` flows through every call.** Provider must refuse cross-tenant
   page handles by construction. Per-tenant credential vault keyed on
   `tenant_id`.
5. **Artifact root confinement.** Screenshots, traces, HARs, downloads land
   under `<sessions_dir>/<tenant_id>/<session_id>/browser/`. Provider rejects
   any path outside this prefix.
6. **Daemon socket is loopback-only.** UNIX socket under per-tenant runtime
   dir. No 0.0.0.0 binding, no remote control, no shared cross-tenant socket.
7. **Re-snapshot after every successful action.** Tools return the new
   snapshot inline so the agent's next turn never has to ask.
8. **`browser_eval` is capability-gated.** Default off. Per-agent
   `[agent.browser] allow_eval = true` to enable. RCE-equivalent — always
   audit-logged.
9. **`check-injection` always-on.** Page text returned to the model passes
   through the gsd-browser-style scanner; matches tag the snapshot in
   `ToolOutcome.details`.
10. **Vendored upstream code carries attribution.** ADR-024 precedent — any
    file vendored from gsd-browser / jcode BPP gets a header pointing back
    to source URL + license + sync date.

## Hybrid (TesterArmy) discipline

`browser_snapshot` first. `browser_scene_understand` and
`browser_vision_click` only when:
- Latest snapshot has zero matches for the agent's described intent, OR
- A `browser_act { ref }` failed with `StaleRef` after a re-snapshot, OR
- `browser_scene_understand` flagged visual elements absent from a11y, OR
- The user explicitly invoked vision (canvas, payment, OAuth, custom widget).

The vision tool calls a separate vision-capable model (default = the agent's
configured chat model if vision-capable; per-agent override
`[agent.browser] vision_model = "..."`). The primary agent never reasons
about coordinates directly.

## Performance budgets

These are **gates**, not asserts — measured by the test plan, not assumed.

| Operation | Target |
|---|---|
| Warm `browser_snapshot` (medium page) | < 150 ms |
| Warm `browser_act` (ref click/type) | < 100 ms |
| `browser_screenshot` | < 500 ms |
| Cold daemon start (Chrome already installed) | < 2 s |
| `browser_vision_click` full round trip | < 5 s |

Regressions on warm-path budgets must block merge.

## Test plan (skeleton)

Unit:
- Tool schemas (each `Tool::parameters()` validates against shipped JSON-Schema spec)
- Ref versioning (stale ref → typed error)
- Action validation (rejects invalid `kind` combos)
- Artifact path confinement (rejects `..` / absolute-path escape)
- Vision result shape (`ToolOutcome.details` carries coords + confidence + bbox)
- `check-injection` matches known patterns + leaves clean text untouched

Integration (local static test server):
- Semantic forms (login, fill, submit, validation error)
- Hidden-but-a11y-visible elements → must filter or warn, not present as truth
- Inaccessible visible buttons → `browser_vision_click` resolves
- Canvas controls → `browser_vision_click` only path
- Modal overlays → snapshot reflects, ref click works
- Iframes / shadow DOM → frames addressable, refs scoped
- File upload → artifact path obeyed
- Network mocking + blocking
- Trace export → har file lands in artifact dir

Agent-loop:
- Tools register in `ToolRegistry`
- `ToolExecUpdate` events emit during long ops (vision delegation)
- `ToolOutcome.details` deterministic across runs

Performance:
- Warm-path budgets above (instrumented; regressions block merge)
- Daemon cold-start budget
- Parallel named sessions (per-tenant isolation under load)

## v0.1 scope

- Native Rust `chromiumoxide` provider only (Chromium / Chrome).
- Default headless in CI, headed in local dev (config flag).
- The 13 structured tools + 2 vision-fallback tools (15 total Tool impls;
  `browser_act` + `browser_wait` + `browser_assert` collapse multiple
  primitives into typed `kind` enums, so the underlying action vocabulary
  is ~25 verbs).
- Per-tenant credential vault (AES-GCM + Argon2 keyed by `tenant_id`).
- Always-on `check-injection`.
- `browser_eval` capability-gated.

## v0.2 backlog

- WebDriver BiDi adapter (Firefox / WebKit coverage without writing those
  drivers).
- `hivecore-browser-playwright-mcp` provider — mounts Microsoft's
  `playwright-mcp` as a backing browser via the v0.2 MCP layer.
- "Attach existing browser" mode (Playwright MCP pattern) — lets the user's
  real Chrome window be the daemon target.
- `browser_visual_diff` — visual regression assertion.
- Recording → `browser_generate_test` (gsd-browser pattern).
- Mobile / device emulation profiles.
- `browser_zoom_region` for high-res inspection of small UI elements.

## Stability

- `BrowserProvider` trait shape (in `hivecore-browser-core`) is the load-bearing
  API. Treat it as semver-binding once v0.1 lands. Capability schema
  evolves additively (new optional capabilities, never breaking existing
  ones).
- Tool surface (`browser_*`) is part of the model-facing API — adding tools
  is fine, removing or renaming is breaking and requires an ADR.
- Persisted artifacts (snapshot JSON, vault files, trace HARs) are forward-
  compatible: readers must tolerate unknown fields (`#[serde(default)]`
  on additions).

## Open questions — RESOLVED 2026-05-01

Resolutions written here so v0.2 implementation has no architectural debt.

1. **Provider trait location** — **`hivecore-browser-core` (Layer 2), pure
   no-I/O.** Matches jcode BPP's transport-agnostic design and lets future
   Playwright-MCP / WebDriver-BiDi adapters implement the same trait
   without depending on `chromiumoxide`.

2. **`browser_act` typed enum vs typed-tool-per-action** — **single tool with
   `BrowserActionKind` enum.** JSON-Schema oneOf in `parameters()`. Keeps
   the model's tool list compact (~17 visible tools instead of ~25).
   Revisit if model grounding suffers — switching to per-action tools is
   purely additive (add new tools, deprecate `browser_act`).

3. **Vision model wiring** — **agent's chat model by default, with explicit
   override via `[agent.browser] vision_model = "..."` in TOML.** If the
   chat model is not vision-capable AND no override is set, vision tools
   return a typed `VisionUnavailable` error rather than failing silently.
   Layer 3 wires the model via dependency injection — `BrowserHarness::new`
   takes an `Option<Arc<dyn ModelAdapter>>` for vision (None = use chat
   model).

4. **Multi-agent fan-out** — **per-(tenant, session) daemon for v0.1.**
   Isolation by construction. Pool optimisation deferred to v0.2 once
   memory-cost data exists. Daemon path:
   `<sessions_dir>/<tenant_id>/<session_id>/browser/daemon.sock`. Daemon
   process exits on session close (lock-file cleanup pattern from
   `hivecore-persistence::lock`).

5. **Cookie / storage state migration** — **Playwright `--storage-state`
   shape** (`{ cookies: [...], origins: [...] }`). Wider interop, broader
   model familiarity. gsd-browser's `save-state/restore-state` pattern
   adopted at the **serialiser level only** — i.e. their write/restore
   workflow informs ours, but on-disk format is Playwright's.

## Additional decisions (resolved with the same pass)

6. **`TenantId` as Layer 1 primitive.** Adding a new newtype to
   `hivecore-runtime-core::ids` is **out of scope** for ADR-028. Instead,
   `tenant_id` flows as a `String` field on `BrowserContext` in
   `hivecore-browser-core`. When ADR-029 (Tenancy plane) lands, `TenantId`
   gets promoted to a Layer-1 newtype and we migrate. Keeps Layer 1
   stable in the interim.

7. **Vault passphrase source.** Three-tier resolution, first hit wins:
   (a) explicit per-tenant `[tenant.<id>] vault_passphrase_env = "VAR"` in
       `~/.hivecore/policy.toml` → resolve via env var
   (b) hivecore-managed file at `<sessions_dir>/<tenant_id>/.vault.key`
       (mode 0600), generated on first session if absent
   (c) operator-supplied via `HIVECORE_VAULT_PASSPHRASE_<TENANT>` env var
   No KMS in v0.1 — landed in v0.2 alongside the Tenancy plane.

8. **`chromiumoxide` version pin.** Pin to `=0.9.x` (matching gsd-browser's
   choice). Track upstream in a v0.2 task; bump together when
   gsd-browser bumps.

9. **CI fixture story.** Local static test server (`tiny_http` + a fixture
   directory of HTML pages: forms, iframes, canvas, modals, redirects,
   network-failure pages). Tests bind to `127.0.0.1:0` (random port).
   No real-internet calls in CI.
2. **`browser_act` typed enum vs typed-tool-per-action** — current plan
   collapses; do we revisit if model grounding suffers from action
   discoverability inside a single tool's schema?
3. **Vision model wiring** — default to agent's chat model if vision-capable,
   or require explicit `[agent.browser] vision_model` config? Recommendation:
   chat-model fallback with explicit override path.
4. **Multi-agent fan-out** — per-tenant single daemon vs per-(tenant,
   session) daemon? gsd-browser is single-daemon; jcode is per-session.
   Recommendation: per-(tenant, session) daemon for isolation; pool
   optimisation lands v0.2.
5. **Cookie / storage state migration** — Playwright `--storage-state` shape
   vs gsd-browser's `save-state/restore-state`? Recommendation: Playwright
   shape (broader interop), with gsd-browser pattern as serialiser
   precedent.
