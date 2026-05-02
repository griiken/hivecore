# HITL prior-art — approval/confirmation primitives across 4 agent runtimes

> Drives a hivecore ADR for the HITL primitive.
> Research date: 2026-05-03. Subjects: pi-mono, OpenAI Codex CLI,
> Zed Agent Client Protocol (ACP), Warp Agent Mode.
> Authority sources cited inline as `[file:line]` or URL. Where tags are
> `[ASSUMED]` the claim is from training-data inference, not verified
> against current code/docs in this session.

---

## TL;DR

Across all four systems, "ask the user before doing X" reduces to **the
same five-question shape**:

1. **What's being approved?** — typed action payload (command, file diff, MCP tool call, network access, …).
2. **Who decides?** — fixed policy per (action-class, scope) plus an optional
   ask-the-user fallback.
3. **What can the user say?** — a small enum: `Approve once`, `Approve always /
   for-session`, `Reject once`, `Reject always`, `Abort turn`.
4. **What's the cache key?** — most use `(class, identity, scope)` where
   identity = the command prefix / tool name / patch path / host, and scope
   ∈ `{call, turn, session, persistent}`.
5. **How does the loop pause?** — async request-response. The runtime suspends
   the call site (or the whole turn), emits a typed event/JSON-RPC request,
   waits on a future for the user's reply, then routes the reply back.

Codex is the most primitive-rich (network/file-system/exec/patch/MCP/elicitation
all share one `GuardianAssessmentAction` shape and one `ReviewDecision`
enum). ACP is the most primitive-poor but ships the wire shape ready-made.
Pi has a UX confirm primitive but no typed approval state machine. Warp
collapses everything to per-command regex allow/deny + four profile
categories.

For hivecore, the right move is: **adopt ACP `RequestPermissionRequest` /
`RequestPermissionResponse` as the wire shape (Layer 1 already speaks ACP
via `hivecore-acp-server`), define an internal `Approval` trait in
`hivecore-runtime-core` whose interpreter lives in `hivecore-agent-loop`,
and let Layer 3 sinks (ACP server, future CLI/web UI) implement it.** The
existing `ToolHook::before` returning `HookOutcome::ManualAttention` is
*almost* the right shape but is currently a one-shot abort path — it needs
to gain a "wait for user, then resume" return mode. Details below.

---

## 1. pi-mono (badlogic/pi-mono)

### Authority sources
- Vendored at `~/.nvm/versions/node/v22.18.0/lib/node_modules/gsd-pi/packages/`.
- Behavioural notes at `.planning/intel/pi-anatomy.md` (this repo).
- Source URLs: <https://github.com/badlogic/pi-mono> · MIT.

### What pi has
Pi exposes **two distinct surfaces** that together do HITL but are not
unified:

1. **`pi.on('tool_call')` event hook** that can return `{block: true, reason}`.
   This is the gate, but it's *programmatic* — meant for extensions ("this
   extension blocks `bash` calls that touch `/etc`"), not for "ask the human
   what they want."
   - Source: `packages/agent/src/types.ts` → `AgentLoopConfig.beforeToolCall`.
   - Notes: `.planning/intel/pi-anatomy.md:22, 58, 92`.
2. **`ctx.ui.confirm/select/input/notify`** — UX primitives an extension can
   call from inside `beforeToolCall` (or anywhere else) to ask the user. Pi
   wires these to the TUI (`packages/tui`) or Web UI (`packages/web-ui`)
   transparently.
   - Source: pi `ExtensionContext` doc, `.planning/intel/pi-anatomy.md:67`.

### State machine
Pi has no first-class approval state machine. The composition is:

```
tool_call event → extension calls ctx.ui.confirm(prompt) → boolean
                ↓
      if false: return {block: true, reason: "user rejected"}
      if true:  return undefined (continue)
```

There are no built-in `allow_always` / `allow_for_session` semantics — each
extension that wants them rolls its own state in `pi.appendEntry`
(non-LLM-visible custom records).

### Cache scope
Per-call only, by default. Persistence is the extension's job
(`pi.appendEntry` writes JSONL into the session log; an extension may
also store to disk under `.pi/state/`).

### UX shape
Pi's UI primitive is rendered by whichever surface is active:
- TUI: `ctx.ui.confirm` blocks the input loop, shows a yes/no prompt.
- Web UI: a modal.
- Extension can choose `confirm`, `select` (multi-option), `input`
  (free-form), `notify` (no answer expected).

### How the agent pauses
**Synchronously, inside the JS event loop.** `tool_call` is awaited; until
the extension's promise resolves, the agent loop's `processToolCalls`
remains parked on `await this.config.beforeToolCall(ctx)`. No message
round-trip — the suspend is in-process.

### What this means for hivecore
- Pi's "events double as gates" pattern was explicitly **rejected** in our
  ADR-022: hivecore separates `LifecycleHook` (gate, returns `Outcome`)
  from `EventSink` (observer, no return). [`.planning/intel/pi-anatomy.md:62-63, 96-97`].
- Pi's `ctx.ui` surface is a **UX primitive abstraction**, not an approval
  abstraction — i.e. it answers "how do I render a yes/no?" not "what's
  the canonical shape of an approval request?". Hivecore needs *both*: a
  typed approval payload (Codex/ACP shape) **and** a UX surface
  (something `ctx.ui`-shaped). Layer 3.
- Adopt: nothing structural. Pi confirms only that the agent-loop pause
  point belongs at `ToolHook::before`, which we already have.

---

## 2. OpenAI Codex CLI (`openai/codex`)

The deepest, most evolved approval system in OSS prior art.

### Authority sources

| File | Lines | Purpose |
|------|-------|---------|
| `codex-rs/protocol/src/approvals.rs` | 1-441 | Approval request events, `ExecPolicyAmendment`, `GuardianAssessmentAction`, `ApplyPatchApprovalRequestEvent`, `ElicitationRequest`. |
| `codex-rs/protocol/src/protocol.rs` | 3643-3699 | `ReviewDecision` enum (the user's answer). |
| `codex-rs/protocol/src/protocol.rs` | 936-1006 | `AskForApproval` enum (top-level mode) + `GranularApprovalConfig`. |
| `codex-rs/protocol/src/request_permissions.rs` | 1-81 | `RequestPermissionProfile` + `PermissionGrantScope` (Turn / Session). |
| `codex-rs/core/src/mcp_tool_call.rs` | (see `.research/mcp-integration.md:243-266`) | `handle_mcp_tool_call`: dispatches MCP approval. |
| `codex-rs/execpolicy/policy.rs` | — | Starlark `prefix_rule` decision engine. |
| `codex-rs/protocol-app-server/...` | — | `ApplyPatchApprovalParams`, `ExecCommandApprovalParams` JSON-RPC envelopes (the wire shape). [Notes: `.planning/intel/codex-patterns.md:202`] |

Verified in this session via `curl https://raw.githubusercontent.com/openai/codex/main/codex-rs/protocol/src/{approvals.rs,protocol.rs,request_permissions.rs}`.

### Approval modes (top-level dial)

Source: `codex-rs/protocol/src/protocol.rs:936-1006`.

```rust
pub enum AskForApproval {
    UnlessTrusted,             // serde "untrusted" — only known-safe read-only auto-OK
    OnFailure,                 // DEPRECATED — sandbox first, escalate on failure
    OnRequest,                 // default — model decides when to ask
    Granular(GranularApprovalConfig), // per-category fine-grained
    Never,                     // never ask; failures returned to model
}

pub struct GranularApprovalConfig {
    pub sandbox_approval: bool,        // shell escalations
    pub rules: bool,                   // execpolicy `prompt` rules
    pub skill_approval: bool,
    pub request_permissions: bool,     // the `request_permissions` tool
    pub mcp_elicitations: bool,
}
```

This is a **policy-shape** dial — it doesn't decide a specific call, it
decides who's allowed to *be asked*. `false` for a category = automatic
deny without surfacing a prompt.

### Approval action taxonomy (the typed payload)

Source: `codex-rs/protocol/src/approvals.rs:134-170`.

```rust
pub enum GuardianAssessmentAction {
    Command  { source, command, cwd },
    Execve   { source, program, argv, cwd },
    ApplyPatch { cwd, files },
    NetworkAccess { target, host, protocol, port },
    McpToolCall { server, tool_name, connector_id, connector_name, tool_title },
    RequestPermissions { reason, permissions: RequestPermissionProfile },
}
```

Six kinds, **one struct**, one wire shape. Critical because every UX
surface (TUI, app-server JSON-RPC, audit log) parses the same enum.

### Approval request events (per kind)

Three wrappers exist over the action enum, each with kind-specific extras:

1. **`ExecApprovalRequestEvent`** (`approvals.rs:211-257`) — for
   `Command`/`Execve`/`NetworkAccess`. Carries:
   - `call_id` (Responses API), optional `approval_id` for sub-execve
     intercepts, `turn_id`, `command`, `cwd`, optional `reason`.
   - `network_approval_context: Option<NetworkApprovalContext>` — present iff
     the prompt is for blocked egress.
   - `proposed_execpolicy_amendment: Option<ExecPolicyAmendment>` — the
     prefix-rule the user *could* persist, if they want this command to
     auto-approve forever.
   - `proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>`.
   - `additional_permissions: Option<AdditionalPermissionProfile>` — extra
     fs/network grants the command will need.
   - `available_decisions: Option<Vec<ReviewDecision>>` — server tells client
     which buttons to render. Falls back to `default_available_decisions()`
     for old clients.
   - `parsed_cmd: Vec<ParsedCommand>` — pre-parsed for syntax-highlighted UX.

2. **`ApplyPatchApprovalRequestEvent`** (`approvals.rs:365-380`) —
   `call_id`, `turn_id`, `changes: HashMap<PathBuf, FileChange>`, optional
   `reason`, optional `grant_root` (asks "may I write under this root for
   the rest of the session?").

3. **`RequestPermissionsEvent`** (`request_permissions.rs:66-80`) —
   `call_id`, `turn_id`, optional `reason`, `permissions:
   RequestPermissionProfile { network, file_system }`, optional `cwd`.
   The model has emitted a structured `request_permissions` tool call
   asking for fs/network grants.

### The user's answer — `ReviewDecision`

Source: `codex-rs/protocol/src/protocol.rs:3643-3699`.

```rust
pub enum ReviewDecision {
    Approved,                                 // this call only
    ApprovedExecpolicyAmendment {             // approve + persist a prefix rule
        proposed_execpolicy_amendment: ExecPolicyAmendment,
    },
    ApprovedForSession,                       // approve + remember for session
    NetworkPolicyAmendment {                  // approve/deny + persist host rule
        network_policy_amendment: NetworkPolicyAmendment,
    },
    Denied,                                   // reject this; agent continues
    TimedOut,                                 // auto review timed out
    Abort,                                    // reject + halt the turn
}
```

Plus, for the `request_permissions` tool, the response carries an explicit
**scope** (`request_permissions.rs:10-16, 56-64`):

```rust
pub enum PermissionGrantScope { Turn, Session }   // default Turn
pub struct RequestPermissionsResponse {
    pub permissions: RequestPermissionProfile,
    pub scope: PermissionGrantScope,
    pub strict_auto_review: bool,             // review every subsequent cmd
}
```

So Codex has **three orthogonal axes** for "approve":
1. *What* is approved (the `permissions` profile, or a `prefix_rule`, or a
   network host rule).
2. *For how long* (this call / this turn / this session / persistent
   amendment).
3. *Under what review regime afterwards* (`strict_auto_review` flips on a
   gate that re-prompts every command, even normally-auto-approved ones).

### Cache keys

By kind:
- **Exec / Execve:** `(prefix_rule_pattern)` — keyed on the *token-prefix*
  of the command. Adding an `ExecPolicyAmendment` writes a Starlark
  `prefix_rule(pattern=[...], decision="allow")` to disk; on next run the
  policy engine matches before the user is even asked.
- **MCP:** `(server_name, tool_name)` for "always allow"; per-tool override
  in `~/.codex/config.toml` `[mcp_servers.X.tools.Y] approval_mode =
  "always"`. Already mirrored in `crates/hivecore-mcp-client/src/config.rs`.
- **ApplyPatch:** `(grant_root)` — once granted, writes under that root for
  the session.
- **Network:** `(host, protocol)` via `NetworkPolicyAmendment` either
  per-session or persistent.
- **request_permissions tool:** `(turn | session, profile)` — the scope is
  explicit on the response.

### UX shape

Codex has **two UX surfaces**, both fed from the same event:

1. **CLI / TUI:** `codex-rs/tui/` renders the `ExecApprovalRequestEvent`.
   The user sees:
   - Pre-parsed command (`parsed_cmd`) with syntax highlighting.
   - Cwd, optional reason.
   - The buttons listed in `available_decisions` (commonly: `Approve`,
     `Approve & remember (prefix-rule preview)`, `Approve for session`,
     `Deny`, `Abort`).
2. **App-server JSON-RPC (`codex-app-server` protocol):** Codex emits
   `ApplyPatchApprovalParams` / `ExecCommandApprovalParams` requests over
   stdio JSON-RPC; the embedder (Zed mounts Codex this way, and Codex's
   own GUI uses it) replies with the `ReviewDecision`. This is Codex's
   alternative to ACP. [`.planning/intel/codex-patterns.md:202`]

### Pause/resume mechanism

**Synchronous async/await with an in-process oneshot.** When `handle_exec`
or `handle_mcp_tool_call` hits a non-auto-approved branch, it:

1. Builds the `ExecApprovalRequestEvent`.
2. `oneshot::channel::<ReviewDecision>()` is created.
3. The event is dispatched onto whatever channel the active surface is
   listening on (CLI render loop / app-server bidirectional stream).
4. The handler `await`s the oneshot.
5. On reply, the decision is matched; on `Approved`/`ApprovedForSession`
   the call proceeds; on `Denied` the model is given a synthetic tool
   error result; on `Abort` the turn halts.

The agent loop itself keeps running — it's blocked at the per-tool-call
await, but other turns/sessions continue. The "pause" is **per-tool-call**,
not per-loop.

### MCP-specific approval flow

From `.research/mcp-integration.md:240-266`:

```
1. Parse args.
2. Look up tool annotations.
3. Run `mcp_permission_prompt_is_auto_approved` against the server's
   permission profile + per-tool override.
4. If not auto-approved, emit `McpToolCallBegin`, send through the
   guardian/elicitation channel.
5. On accept, RmcpClient::call_tool; emit `McpToolCallEnd`.
6. Cache "always-allow" choice keyed `(server, tool)` for the session
   (or persistently if the user picked the "remember" decision).
```

Hooks (`run_permission_request_hooks`) run *before* the user prompt and
can intercept (extension auto-approves on policy match). This is the
hivecore Layer 3 hook surface.

### What hivecore takes from Codex
1. **Exhaustive typed action enum.** `GuardianAssessmentAction` covers
   every flavour of HITL with one shape. Hivecore's `ApprovalAction` will
   mirror it — but add `tenant_id` (multi-tenancy ADR-019) and avoid the
   `RequestPermissions` recursive variant for v0.1 (it's a tool that asks
   for more tools' permissions; defer).
2. **`ReviewDecision` shape verbatim.** Five outcomes (`Approved`,
   `ApprovedForSession`, `Denied`, `TimedOut`, `Abort`) plus two amendment
   variants. Drop `ApprovedExecpolicyAmendment` until hivecore has an
   execpolicy engine; keep the rest.
3. **Three-axis approval (what / for-how-long / under-what-regime).** The
   `PermissionGrantScope { Turn, Session }` axis is genuinely useful; copy
   it. `strict_auto_review` is a v0.2 idea (escalation tightening).
4. **`available_decisions` pattern.** Server picks which buttons to show;
   client renders them. Avoids the client having to know which decisions
   are valid for which action kinds. Hivecore should ship this from day one.
5. **`AskForApproval` top-level dial.** Adopt `OnRequest` as default,
   `Never` for autonomous workflows, `UnlessTrusted` for read-only-only.
   Skip `OnFailure` (deprecated even in Codex). `Granular` is a v0.2
   refinement.
6. **Self-amending policy via approvals.** When the user picks
   "approve & remember", record a typed amendment in the audit log and
   refresh the in-memory policy. This is the cleanest "persistent
   approval" model.

### What hivecore doesn't take (yet)
- **Starlark execpolicy engine.** ADR-007 commits us to TOML for
  config-shape rules. Reserve Starlark/Rhai for v0.5 gate expressions.
- **`request_permissions` tool variant.** Recursive permission tool is
  powerful but raises the complexity floor; v0.2.
- **Network-level approvals as a first-class kind.** Hivecore's network
  policy belongs in the sandbox plane (Layer 3 / future
  `hivecore-sandbox-*` crates), not the runtime-level approval primitive.
  Defer.
- **Codex's 6-kind `GuardianAssessmentAction` enum.** Start with three:
  `Tool`, `ApplyPatch`, `Mcp`. Add `Network` when sandbox lands.

---

## 3. Zed Agent Client Protocol (ACP)

The wire-shape we already speak via `crates/hivecore-acp-server`. ADR-018.

### Authority sources

- **Schema:** `https://raw.githubusercontent.com/zed-industries/agent-client-protocol/main/schema/schema.json`
  (verified in this session, 152 KB).
- **Spec docs:** `https://agentclientprotocol.com/protocol/permissions` (cited in this session via inferred references; full page verified by schema cross-check).
- **In-tree:** `crates/hivecore-acp-server/src/server.rs:1-279`, `bridge.rs`, `lib.rs`.

### Wire shape — `session/request_permission`

Method: `session/request_permission` (JSON-RPC, `x-side: client` — agent
sends to client).

`RequestPermissionRequest`:

```json
{
  "sessionId": "<SessionId>",
  "toolCall":  { "toolCallId": "...", /* ToolCallUpdate fields */ },
  "options":   [PermissionOption, ...]
}
```

`PermissionOption`:
```json
{ "optionId": "<opaque-string>", "name": "Allow once", "kind": "allow_once" }
```

`PermissionOptionKind` enum (verbatim from schema):
- `allow_once` — "Allow this operation only this time."
- `allow_always` — "Allow this operation and remember the choice."
- `reject_once` — "Reject this operation only this time."
- `reject_always` — "Reject this operation and remember the choice."

`RequestPermissionResponse`:
```json
{ "outcome": { "outcome": "selected", "optionId": "..." } }
// or
{ "outcome": { "outcome": "cancelled" } }
```

The `cancelled` outcome is mandatory: when the client sends
`session/cancel`, it MUST respond `cancelled` to all pending
`session/request_permission` requests.

### Tool call lifecycle (the surrounding context)

`ToolCallStatus` (verbatim from schema):
- `pending` — "hasn't started running yet because the input is either
  streaming or we're awaiting approval."  ← the "awaiting approval" state.
- `in_progress`
- `completed`
- `failed`

`ToolKind`: `read`, `edit`, `delete`, `move`, `search`, `execute`,
`think`, `fetch`, `switch_mode`, …

`ToolCallUpdate` carries `toolCallId`, optional `status`, `kind`,
`title`, `content[]`, `locations[]`, `rawInput`, `rawOutput`. The agent
sends a `ToolCallUpdate` with `status: "pending"` to put a tool in the
"awaiting approval" state, then `session/request_permission`, then
either `status: "in_progress"` (on approval) or `status: "failed"` (on
rejection / cancel).

### What ACP gives us

- **Wire format already done.** No design work needed for the JSON-RPC
  envelope.
- **Four-option vocabulary** (`allow_once`/`allow_always`/`reject_once`/
  `reject_always`) which is **strictly less expressive** than Codex's
  `ReviewDecision` enum but covers 80 % of cases.
- **`optionId` is opaque** — the agent can stuff anything in there, so
  hivecore can encode kind-specific extras (e.g., a Codex-style
  `ApprovedExecpolicyAmendment` could become an `optionId =
  "approve-and-pin-prefix:git status"`). The client just round-trips it.
- **Cancellation semantics built in.** `session/cancel` ⇒ pending
  permission requests get `cancelled` outcome.

### What ACP doesn't give us

- **No typed scope (`turn` vs `session` vs `persistent`).** The
  `allow_always` option means "remember the choice" but doesn't say
  *for whom* or *for how long*. This is by design — the agent decides
  what "remembering" means and persists it server-side. Hivecore needs
  its own scope axis on top.
- **No typed action payload.** ACP's `ToolCallUpdate` carries
  `toolCallId` + `kind` + free-form `rawInput`. Codex-style
  "this is a `Command` with `command: ["rm", "-rf", "x"]`" is left to
  the agent to encode in `rawInput` or in `content[]`. Hivecore should
  publish a stable in-tree action enum and serialize it into ACP's
  open slots.
- **No native amendment / proposed-rule shape.** The "remember the
  choice" is opaque; if hivecore wants Codex-style "here's a prefix rule
  we'll add to your policy", it goes in `optionId` (encoded) or in
  `_meta`.

### Pause/resume mechanism

JSON-RPC request — the agent sends `session/request_permission` and
`await`s the response. Same in-process oneshot pattern as Codex, but
across the wire. The session's prompt-turn state machine waits;
`session/cancel` can interrupt.

In `hivecore-acp-server/src/server.rs` today this isn't wired — the
`AgentLoop` runs to completion in `run_prompt_turn` without any
permission round-trip. Adding HITL means:

1. A `Permission` trait on the runtime side (Layer 1) that the loop
   `await`s.
2. An ACP-side adapter that translates the trait call to
   `session/request_permission` and back.
3. A non-ACP fallback (CLI prompt over stdin) for `hivecore-coder`.

### What hivecore takes from ACP
1. **Wire shape.** `session/request_permission` is what we implement on
   the ACP server side. No reinvention.
2. **Four-option vocabulary as the *minimum* set.** Always render at least
   these four buttons; richer kinds (Codex-style "approve and pin a prefix
   rule") become extra `PermissionOption` entries with custom `optionId`
   and `kind: "allow_once"` (no extra kind values exist in ACP).
3. **`pending` status as the loop pause-point indicator.** When a tool is
   awaiting approval, the agent has emitted `ToolCallUpdate { status:
   pending }` already — UI surfaces can show a spinner/diff/preview while
   the user decides.
4. **Cancellation handling.** `session/cancel` ⇒ cancel pending approvals.
   Hivecore's `AbortSignal` already plumbs this; we just need to
   wire it into the approval future.

---

## 4. Warp Agent Mode

Lightest-weight prior art, but the most opinionated UX.

### Authority sources
- <https://docs.warp.dev/agents/autonomy/agent-permissions>
- <https://docs.warp.dev/agents/using-agents/agent-profiles-permissions>
- WebSearch summary verified in this session (2026-05-03). Warp's docs
  site is JS-rendered, so URL-fetch returns shell HTML; substantive
  content reproduced via search-engine indexed snippets.
- `.planning/intel/warp-actions.md` (typed-action protocol — pairs with
  permissions).

### Approval model

Warp organises permissions per **profile** (Settings → AI → Agents →
Profiles → Permissions). Each profile sets a default `Always allow /
Allow once / Always reject` per **action category**:

- Run commands
- Edit files
- Apply (write) AI suggestions
- Use MCP tools
- Use built-in tools
- Use the web

These are **the same coarse buckets as Codex's `GranularApprovalConfig`**
(`sandbox_approval`, `rules`, `skill_approval`, `request_permissions`,
`mcp_elicitations`) — different names, same shape.

### Per-command granularity (the Warp innovation)

Two regex lists override the profile defaults:

- **Command allowlist** (`Settings → AI → Agents → Command allowlist`):
  user-defined regexes; matching commands auto-execute even if the
  profile would normally prompt.
- **Command denylist**: user-defined regexes; matching commands always
  prompt, **even in YOLO mode** ("when all permissions are set to Always
  allow"). Denylist beats allowlist.

This is **Warp's equivalent of Codex's `ExecPolicyAmendment`** but
authored by humans, not produced by the agent's per-call approve flow.
No "approve once and persist" UX in Warp's docs — list editing is
manual.

### Risk metadata (set by the model, not the user)

From `.planning/intel/warp-actions.md:74-82`: every shell-ish action
variant carries `is_read_only`, `is_risky`, `wait_until_completion`,
`uses_pager`, `rationale`, `citations`. The profile's "Allow once" mode
uses these to decide whether to prompt — `is_read_only=true` →
auto-execute; `is_risky=true` → always prompt regardless.

### Cache scope

Allowlist/denylist regexes are persistent (per profile, per workspace).
Profile selections are persistent. There's no per-session "remember"
cache visible in the docs.

### UX shape

Before a command runs, the user sees:
- Command preview (with the model's `rationale`).
- "Run" / "Reject" buttons (or auto-runs if allowlist matched).
- Citations (if the model attached source URLs).

The "preview before run" UX with `rationale + citations` is a Warp
distinctive — pi/Codex/ACP show the command but typically not the
model's reasoning prose.

### Pause/resume mechanism

Inferred from action enum + UI behaviour: the agent emits an action via
the typed `AIAgentActionType` channel, the UI inspects `is_read_only` /
`is_risky` against the current profile + allowlist/denylist, and either
auto-executes or shows the prompt and waits for the click. The action is
a future the orchestrator awaits. **`[ASSUMED]`** — Warp source not
inspected in this session.

### What hivecore takes from Warp
1. **Risk metadata baked into the action.** `is_read_only`,
   `is_risky`, `rationale`, `citations` — already adopted via ADR-018.
   These flags are *inputs* to the approval decision, not the decision
   itself. Hivecore: action carries metadata; policy decides whether it
   needs HITL given the metadata.
2. **Per-command regex allow/deny.** Cleanest way to let admins
   *manually* curate "always-OK" commands without going through an
   approve-and-pin flow. Hivecore policy file (`.hivecore/policy.toml`)
   should support this from v0.1, alongside the agent-driven amendment
   pattern from Codex.
3. **Profile-scoped permission categories.** Hivecore agent config
   (`.hivecore/agents/<name>.toml`) already has a `[limits]` table; add
   `[permissions]` with the same coarse buckets (run-commands, edit-files,
   mcp-tools, …). Categories beat per-tool config for human-edited files.

### What hivecore doesn't take from Warp
- **No "approve and persist via UI" flow.** Warp leaves persistence to
  manual list editing. Hivecore should support both paths (Codex amendment
  + Warp manual list).
- **No JSON-RPC wire shape.** Warp is a closed app; the action enum is
  the only public surface.

---

## 5. Synthesis — what they all agree on

Stripping away the names, all four converge on this state machine:

```
       ┌────────────────────────────────────────────────┐
       │       agent loop reaches a tool/patch call     │
       └───────────────┬────────────────────────────────┘
                       │ build typed ApprovalAction
                       ▼
       ┌────────────────────────────────────────────────┐
       │           policy engine evaluates              │
       │  inputs: action, profile, persistent rules,    │
       │           session cache, action metadata       │
       └───────────────┬────────────────────────────────┘
                       │
              ┌────────┼────────┐
              │        │        │
         AutoAllow  AutoDeny  AskUser
              │        │        │
              │        │        ▼
              │        │   emit ApprovalRequest event
              │        │   suspend on oneshot<Decision>
              │        │   ────────────────────────
              │        │   user picks one of:
              │        │     ApproveOnce / ApproveAlways(scope)
              │        │     RejectOnce / RejectAlways(scope) / Abort
              │        │   optionally: amend persistent rules
              │        │   ────────────────────────
              │        │        │
              │        │   ┌────┴────┬────────┐
              │        │   │         │        │
              │        │ Allow     Reject   Abort
              ▼        ▼   ▼         ▼        ▼
           execute  syn-error  execute  syn-error  halt turn
                      back to                       back to
                      model                         user
```

Common facts:

| Concern | All four converge on … |
|---------|------------------------|
| Action shape | Typed enum with a `kind` discriminant |
| Decision shape | At minimum: `approve-once`, `approve-persist`, `reject-once`, `abort` |
| Persistence | Some form of (action-class, identity) → decision cache |
| Scope axis | At least two of {call, turn, session, persistent} |
| Render hint | UI gets enough metadata (kind + action body) to render usefully without parsing prose |
| Pause | Async oneshot; `cancel` cancels pending approvals |
| Auto-approve | Policy + per-call metadata + cached decisions |

Differences:

| Concern | Pi | Codex | ACP | Warp |
|---------|----|-------|-----|------|
| Typed action kinds | none | 6 (`Command`/`Execve`/`Patch`/`Network`/`Mcp`/`RequestPermissions`) | open (`ToolKind`+`rawInput`) | many (full `AIAgentActionType` enum) |
| Decision enum size | 2 (block/allow) | 7 (`ReviewDecision`) | 4 (`PermissionOptionKind`) | 3 (`Always allow`/`Allow once`/`Always reject`) |
| Scope axis | extension-rolled | explicit `Turn`/`Session` + persistent amendments | implicit ("remember") | persistent profile + regex lists |
| Wire format | in-process JS | JSON-RPC (`codex-app-server`) | JSON-RPC (`session/request_permission`) | proprietary |
| Auto-approve from metadata | no | yes (`is_safe_command`) | n/a | yes (`is_read_only`/`is_risky`) |
| Self-amending policy | no | yes (`ExecPolicyAmendment`) | no | no (manual list) |
| Per-tool regex/prefix lists | no | execpolicy `prefix_rule` | no | regex allowlist/denylist |

---

## 6. Hivecore design recommendation

### 6.1 Trait surface — Layer 1 (`hivecore-runtime-core`)

A new module `approval.rs` adds **one trait** + **one typed payload** +
**one decision enum**. I/O-free.

```rust
// hivecore-runtime-core/src/approval.rs

/// What the runtime is asking the human about. Mirrors Codex's
/// `GuardianAssessmentAction` minus the variants we defer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAction {
    /// A `Tool::execute` call.  Identity = (tool_name, hash(input)).
    Tool {
        tool_name: String,
        invocation_id: ToolCallId,
        input_preview: serde_json::Value,
        risk: RiskHint,
    },
    /// A workspace-mutating patch.  Identity = (root, file-paths).
    ApplyPatch {
        root: PathBuf,
        files: Vec<PathBuf>,
        // serialized FileChange map; opaque to Layer 1
        changes: serde_json::Value,
    },
    /// MCP tool call (kept distinct from `Tool` for routing — same data,
    /// different identity for caching).
    Mcp {
        server: String,
        tool_name: String,
        input_preview: serde_json::Value,
    },
}

/// Coarse risk hint set by the action emitter (Layer 2/3 tools).  The
/// approval-policy engine consumes this; the user never sees it.
/// Mirrors Warp's `is_read_only`/`is_risky`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RiskHint {
    pub read_only: Option<bool>,
    pub risky: Option<bool>,
    pub network: Option<bool>,
}

/// What the user picked.  Subset of Codex `ReviewDecision`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    ApprovedForSession,                     // remember for this session
    ApprovedAndPersist { rule: ApprovalRule }, // emit a typed amendment
    Denied,                                 // model continues, not aborted
    TimedOut,                               // policy: treat as Denied
    Abort,                                  // halt the turn
    Cancelled,                              // session/cancel race (ACP shape)
}

/// A typed, human-readable amendment.  Variant per identity-kind.
/// Persisted by the audit/policy plane (ADR-019).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRule {
    ToolPrefix { name_prefix: String },
    McpToolAllow { server: String, tool: String },
    PathRoot { root: PathBuf, write: bool },
}

/// Scope axis (Codex's `PermissionGrantScope`, plus a `Call` element so
/// the type can also represent a one-shot Approved decision).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope { #[default] Call, Turn, Session }

#[derive(Debug, Clone)]
pub struct ApprovalRequest<'a> {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub action: &'a ApprovalAction,
    pub reason: Option<String>,
}

/// The single trait Layer 3 implements.  Layer 2's driver awaits this.
#[async_trait]
pub trait Approval: Send + Sync {
    async fn request(&self, req: ApprovalRequest<'_>) -> ApprovalDecision;
}
```

That's it for Layer 1 — five types, one trait. No wire format, no I/O.

### 6.2 Driver wiring — Layer 2 (`hivecore-agent-loop`)

`AgentLoop` gains an optional `Arc<dyn Approval>`. The pause point is in
`dispatch_tools` (driver.rs line ~330), **before** `execute_tool`:

```rust
// pseudo-patch to driver.rs::dispatch_tools
let pre = self.run_pre_hooks(turn_id, &invocation).await;
let outcome = match pre {
    HookDecision::Pass => {
        // *** new: ask the approval source if one is wired ***
        let approval = self.maybe_request_approval(turn_id, &invocation).await?;
        match approval {
            ApprovalDecision::Approved | ApprovalDecision::ApprovedForSession |
            ApprovalDecision::ApprovedAndPersist { .. } => {
                self.execute_tool(turn_id, &invocation, signal.clone()).await?
            }
            ApprovalDecision::Denied | ApprovalDecision::TimedOut => {
                ToolOutcome::error(format!("denied by user: {}", invocation.name))
            }
            ApprovalDecision::Abort | ApprovalDecision::Cancelled => {
                return Err(LoopError::HookAborted("user aborted".into()));
            }
        }
    }
    HookDecision::Override(o) => o,
    HookDecision::FailedContinue(reason) => ToolOutcome::error(reason),
    HookDecision::FailedAbort(reason) => return Err(LoopError::HookAborted(reason)),
    HookDecision::ManualAttention(reason) => return Err(LoopError::ManualAttention(reason)),
};
```

`maybe_request_approval` consults the policy plane (which itself can
auto-decide and skip the human round-trip — Codex's
`mcp_permission_prompt_is_auto_approved` is the inspiration). When the
policy returns `AskUser`, the driver `await`s `Approval::request`. Side
effects (caching `ApprovedForSession`, persisting `ApprovedAndPersist`)
happen at the policy plane, not in the driver.

### 6.3 Policy plane — Layer 3 (new crate `hivecore-approval-policy`)

A small crate that owns:
- The TOML schema (`[approval]` block in agent + workspace config).
- Auto-decision rules: `RiskHint::read_only=true` → auto-allow when
  policy mode is `unless_trusted`; allowlist regex match → auto-allow;
  denylist regex match → mandatory prompt.
- The session-scoped cache (`HashMap<ApprovalKey, ApprovalDecision>`).
- The persistent amendment log (write-through to audit JSONL ADR-019).

Public API:

```rust
pub struct ApprovalPolicy { /* config + cache */ }
impl ApprovalPolicy {
    pub fn evaluate(&self, action: &ApprovalAction, risk: &RiskHint)
        -> PolicyOutcome;  // AutoAllow | AutoDeny | AskUser
    pub fn record(&self, action: &ApprovalAction, decision: &ApprovalDecision);
}
```

The driver calls `evaluate` first; only `AskUser` triggers the
`Approval::request` await.

### 6.4 ACP wiring — `hivecore-acp-server`

Implement the `Approval` trait by translating to ACP
`session/request_permission`:

```rust
struct AcpApproval { /* JSON-RPC peer handle */ }

#[async_trait]
impl Approval for AcpApproval {
    async fn request(&self, req: ApprovalRequest<'_>) -> ApprovalDecision {
        let options = build_options(&req);   // 4 standard + amendment-specific
        let acp_req = RequestPermissionRequest {
            session_id: req.session_id.into(),
            tool_call: ToolCallUpdate { /* status: pending, kind, raw_input */ },
            options,
        };
        let resp: RequestPermissionResponse = peer.send_request(acp_req).await;
        match resp.outcome {
            RequestPermissionOutcome::Cancelled => ApprovalDecision::Cancelled,
            RequestPermissionOutcome::Selected { option_id } => {
                decode_option_id(option_id)   // round-trips encoded scope
            }
        }
    }
}
```

`build_options` always emits the four ACP standard kinds
(`allow_once`/`allow_always`/`reject_once`/`reject_always`); for
amendment-bearing actions (e.g., `ApplyPatch` with a `grant_root`
candidate), it appends a 5th option whose `kind: "allow_always"` and
whose `optionId` encodes the typed `ApprovalRule`. The decoder restores
the rule on response.

### 6.5 CLI wiring — `hivecore-coder`

Implement `Approval` over stdin/stdout, four-options shape, no JSON-RPC.

### 6.6 Composition with `LifecycleHook` (ADR-022)

`Approval` is **not** a `LifecycleHook`. Two reasons:

1. **Different return shape.** Lifecycle hooks return
   `LifecycleOutcome { Pass | FailedContinue | FailedAbort |
   ManualAttention }`. Approval returns a structured `ApprovalDecision`
   that includes "approve and remember" with a typed rule payload.
   Squashing that into `ManualAttention(String)` would lose the typed
   payload.
2. **Different timing.** Lifecycle hooks fire at session/turn boundaries;
   approval fires *between* `pre_hook` and `execute_tool` for *each*
   tool call. A new fan-out point.

But: `ToolHook::before` returning `HookOutcome::ManualAttention` should
**still work** — it's the escape hatch when a hook wants to demand HITL
without going through the typed approval pipeline. The driver can map
`ManualAttention` to `ApprovalAction::Tool { reason: <string> }` and
route through the same `Approval::request`.

### 6.7 Audit (ADR-019 plane)

Every approval round-trip emits two new `AgentEvent` variants:

```rust
AgentEvent::ApprovalRequested { request_id, action, scope_eligible: Vec<ApprovalScope>, at }
AgentEvent::ApprovalDecided   { request_id, decision, scope: ApprovalScope, at }
```

Persistent amendments (`ApprovalRule`) land in the audit JSONL with their
own `Custom { kind: "approval_amendment", … }` entry. Replay = re-load
amendments → seed `ApprovalPolicy.persistent_rules` → identical
auto-decisions.

### 6.8 Multi-tenancy

`ApprovalRequest` already carries `session_id`; the policy cache is
keyed by `(tenant_id, session_id, action_key)` at the Layer 3 boundary
(tenant injected from the session). Persistent amendments are
tenant-scoped — no cross-tenant rule leakage.

### 6.9 v0.1 vs v0.2 scope

**v0.1 (in scope)**
- `ApprovalAction { Tool, ApplyPatch, Mcp }` (3 variants).
- `ApprovalDecision` with all 7 outcomes; default policy maps `TimedOut`
  → `Denied`.
- `ApprovalScope { Call, Turn, Session }` (no persistent amendments yet
  — until the audit/policy plane lands).
- `ApprovalPolicy` evaluating: top-level mode (`OnRequest`/`Never`/
  `UnlessTrusted`) + per-tool `approval_mode = always|never|on_request`
  (already in `crates/hivecore-mcp-client/src/config.rs:91-100`,
  generalize).
- `Approval` trait + ACP impl + CLI impl.
- Audit `ApprovalRequested` / `ApprovalDecided` events.

**v0.2 (backlog)**
- `ApprovedAndPersist` with typed `ApprovalRule` (Codex-style amendment).
- Network kind (`ApprovalAction::Network`) when sandbox plane lands.
- `Granular` policy mode (per-category buckets — Codex shape).
- Persistent regex allowlist/denylist (Warp shape) in
  `.hivecore/policy.toml`.
- `strict_auto_review` gate (Codex `RequestPermissionsResponse`).
- `request_permissions` recursive tool variant (model asks for more
  fs/network grants mid-call).
- Replay of persistent amendments through audit JSONL.

### 6.10 Why this composes well with what already exists

- **`hivecore-runtime-core` stays I/O-free.** `Approval` is a trait, not
  an impl. ✓ CLAUDE.md rule.
- **`hivecore-agent-loop` driver.rs gains one `await` site.** No
  restructuring of the inner/outer loop. The pause point is at the same
  layer as `ToolHook::before` — fits the existing model.
- **`hivecore-acp-server` already has a JSON-RPC peer.** Adding
  `session/request_permission` is incremental.
- **`hivecore-mcp-client` already has `ApprovalMode { Always | Never |
  OnRequest }`.** That's the same enum as the proposed top-level dial —
  extract to a shared crate and the existing MCP code drops into the new
  pipeline unchanged. The `effective_approval_mode(tool)` method
  (`config.rs:172-178`) becomes the per-action evaluator.
- **ADR-022 lifecycle separation is preserved.** Approval is its own
  trait; `LifecycleHook` and `ToolHook` keep their semantics.
- **ADR-019 audit plane absorbs amendments cleanly.** Two new events,
  one new custom-message kind. No new persistence target.

### 6.11 Open questions for the ADR

1. **Where is the policy plane code's home crate?** Three options:
   - `hivecore-runtime-core` (pure types, no I/O — but the cache + policy
     evaluator wants config and is Layer 3-shaped).
   - New `hivecore-approval-policy` crate at Layer 3.
   - Subsumed under `hivecore-config` (which already houses the agent
     TOML schema).
   Recommendation: new `hivecore-approval-policy` Layer 3 crate;
   `hivecore-config` declares the `[approval]` schema, `-policy` consumes
   it. Mirrors how `hivecore-compaction` is split from
   `hivecore-runtime-core`.
2. **`ApprovalScope::Persistent` — encode where?** Defer to v0.2;
   placeholder in the enum or none?  Recommendation: leave it out of v0.1
   so the four-element ACP `PermissionOptionKind` doesn't grow ambiguous
   mappings. Add at v0.2 with the amendment work.
3. **Policy auto-approve for `RiskHint::read_only=true` — opt-in or
   default?** Codex defaults yes (`UnlessTrusted` mode). Warp defaults
   yes if profile permits. Recommendation: default no in v0.1
   (`OnRequest` covers everything); add a `[approval] auto_approve_read_only = true` knob v0.2 once the risk-hint plumbing is exercised.
4. **Does `Approval::request` see the full `ToolInvocation` or just a
   preview?** Full risks leaking large payloads to UIs that don't redact.
   Preview risks losing context for nuanced approvals. Recommendation:
   pass full payload, mark large fields with size hint, let UI decide
   how much to render.
5. **`ApprovalRequested` event — visible to the model on resume?**
   Default no (the event is for human/audit). On replay it's purely a
   journal entry. Mark `visible_to_model: false`.

---

## 7. Cross-system reference table

| | pi-mono | Codex | ACP | Warp |
|---|---|---|---|---|
| **Typed action enum?** | ✗ | ✓ `GuardianAssessmentAction` (6) | open (`ToolKind` + raw) | ✓ `AIAgentActionType` (~25) |
| **Decision enum?** | ✗ (block/allow) | ✓ `ReviewDecision` (7) | ✓ `PermissionOptionKind` (4) | ✓ profile (3) |
| **Scope axis?** | extension | `Turn`/`Session` + persistent amendment | `once`/`always` | profile (persistent) + per-call override |
| **Cache key shape?** | extension-rolled | `(prefix-rule)` / `(server, tool)` / `(grant_root)` / `(host, proto)` | opaque (`optionId`) | regex |
| **Self-amending from approve?** | ✗ | ✓ `ApprovedExecpolicyAmendment` | ✗ | ✗ |
| **Manual allow/deny lists?** | ✗ | ✓ TOML `enabled_tools`/`disabled_tools` | ✗ | ✓ regex allowlist/denylist |
| **Risk metadata?** | ✗ | ✓ via execpolicy + `is_safe_command` | ✗ | ✓ `is_read_only`/`is_risky`/`rationale` |
| **Wire format?** | in-process | JSON-RPC `codex-app-server` | JSON-RPC `session/request_permission` | proprietary |
| **Pause mechanism?** | sync await | async oneshot + JSON-RPC | async JSON-RPC + `session/cancel` | UI button |
| **MCP-aware?** | ✗ (no MCP) | ✓ `mcp_permission_prompt_is_auto_approved` | ✓ via `ToolKind` | ✓ separate category |
| **Multi-tenant?** | ✗ | single-user | ✗ (session-scoped) | per-workspace profiles |

---

## 8. Sources of authority (consolidated)

### Codex (Apache-2.0, verified 2026-05-03)
- `codex-rs/protocol/src/approvals.rs` — full file at `/tmp/codex_approvals.rs` (15.4 KB, 441 lines).
- `codex-rs/protocol/src/protocol.rs:936-1006, 3643-3699` — `AskForApproval`, `GranularApprovalConfig`, `ReviewDecision`.
- `codex-rs/protocol/src/request_permissions.rs` — `RequestPermissionProfile`, `PermissionGrantScope`.
- Behavioural narrative: `.research/mcp-integration.md:240-266`, `.planning/intel/codex-patterns.md`.
- Inspiration crate: `codex-rs/execpolicy/`.

### ACP (Zed, Apache-2.0, verified 2026-05-03)
- `schema/schema.json` — `RequestPermissionRequest`, `RequestPermissionResponse`, `RequestPermissionOutcome`, `SelectedPermissionOutcome`, `PermissionOption`, `PermissionOptionKind`, `ToolCallUpdate`, `ToolCallStatus`, `ToolKind`. (Saved at `/tmp/acp_schema.json`, 152 KB.)
- Method names: `session/request_permission` (`x-method` extension), `session/cancel`.
- Spec docs: <https://agentclientprotocol.com/protocol/permissions> (page exists; this session verified content via schema cross-check).

### Pi-mono (MIT)
- `.planning/intel/pi-anatomy.md:22, 58, 67, 92, 96-97`.
- `packages/agent/src/types.ts` — `AgentLoopConfig.beforeToolCall`.
- `packages/coding-agent/docs/extensions.md` — `ExtensionContext.ui.{confirm,select,input,notify}`.

### Warp
- Verified via WebSearch 2026-05-03; docs site is JS-rendered and not
  directly fetchable.
- <https://docs.warp.dev/agents/autonomy/agent-permissions>
- <https://docs.warp.dev/agents/using-agents/agent-profiles-permissions>
- `.planning/intel/warp-actions.md:74-82, 22-71` — risk metadata + action enum (verified earlier in repo intel work).
- Source code (`warpdotdev/warp` AGPL-3.0 + MIT) **not inspected this session**: claims about Warp's pause mechanism marked `[ASSUMED]` above.

### Hivecore in-tree (this repo)
- `crates/hivecore-runtime-core/src/{tool,hook,lifecycle,event}.rs`.
- `crates/hivecore-agent-loop/src/driver.rs:1-624` — pause-point location
  for the new `Approval::request` await is line ~339-350 (`HookDecision`
  match arm).
- `crates/hivecore-mcp-client/src/config.rs:79-100, 172-178` — existing
  `ApprovalMode` enum + `effective_approval_mode` resolver.
- `crates/hivecore-acp-server/src/server.rs:1-279` — current
  ACP-skeleton; the place where `Approval` adapter is added.

---

## 9. Confidence

| Claim | Confidence | Why |
|-------|-----------|-----|
| Codex types/enums quoted verbatim | HIGH | Source files fetched + read in this session. |
| ACP request/response shapes | HIGH | Schema fetched + parsed in this session. |
| Pi-mono "events double as gates" | HIGH | Already documented in `.planning/intel/pi-anatomy.md` + cross-checked. |
| Warp profile/category model | MEDIUM | Docs site JS-rendered; relied on WebSearch indexed snippets. Field names assumed. |
| Warp pause mechanism | LOW (`[ASSUMED]`) | Source not inspected; inferred from action-enum shape. |
| Hivecore composition recommendation | HIGH | Current code paths verified directly. |
| `[ASSUMED]` claims | LOW | Marked inline. Need user/maintainer confirmation before locking ADR. |

---

## 10. Open assumptions to resolve before ADR

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | Warp pause mechanism is async oneshot per action | §4 | Low — affects narrative only, not hivecore design. |
| A2 | ACP `_meta` is the right channel for amendment payloads (vs encoded `optionId`) | §3, §6.4 | Medium — encoding choice affects ACP-spec extension surface. |
| A3 | `OnRequest` is the right v0.1 default (vs `UnlessTrusted`) | §6.9 | Medium — UX-shaping. |
| A4 | `Approval` belongs in `hivecore-runtime-core` (vs new core trait crate) | §6.1 | Low — refactor cost only. |
| A5 | Audit `ApprovalRequested`/`Decided` events are visible to model on replay | §6.7 | Medium — affects how prior approvals influence future model behaviour. |
