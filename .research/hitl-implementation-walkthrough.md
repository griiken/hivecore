# HITL implementation walkthrough — pi-mono + Codex → hivecore

Companion to `.research/hitl-prior-art.md` (which covers types). This file shows the **actual code** at the loop / hook / UI seam, then maps it onto hivecore.

> Sources read: pi-mono installed at `~/.nvm/.../gsd-pi/packages/{pi-agent-core,pi-coding-agent}` and Codex `codex-rs/core/src/{approvals,safety,mcp_tool_call,protocol}.rs` fetched via `raw.githubusercontent.com/openai/codex/main`. `codex.rs` itself returned 404 at that path (file moved or split), so the turn-loop quote in §2a is best-effort from `mcp_tool_call.rs` and `protocol.rs`.

---

## 1. pi-mono — programmatic block via `beforeToolCall`

pi-mono does **not** prompt a human inline. It exposes `beforeToolCall(ctx) → {block, reason}` and the **extension** decides. The extension can in turn drive a UI (TUI / web), but the loop only sees a synchronous boolean. This keeps the loop pure and shoves UX into the harness layer.

### 1a. The hook contract — `pi-agent-core/src/types.ts:38-41`

```ts
/**
 * Returning `{ block: true }` prevents the tool from executing.
 * The loop emits an error tool result instead.
 */
export interface BeforeToolCallResult {
    block?: boolean;
    reason?: string;
}
```

### 1b. The loop awaits the hook — `pi-agent-core/src/agent-loop.ts:693-712`

```ts
try {
    const validatedArgs = validateToolArguments(tool, toolCall);
    if (config.beforeToolCall) {
        const beforeResult = await config.beforeToolCall(
            { assistantMessage, toolCall, args: validatedArgs, context: currentContext },
            signal,
        );
        if (beforeResult?.block) {
            return {
                kind: "immediate",
                result: createErrorToolResult(beforeResult.reason || "Tool execution was blocked"),
                isError: true,
            };
        }
    }
    return { kind: "prepared", toolCall, tool, args: validatedArgs };
}
```

Control flow: hook returns `{block:true}` → loop short-circuits to an immediate error `ToolResult` and continues. The model sees a regular tool error on the next turn. **No human dialog at the loop level.** Reason text is fed verbatim to the model.

### 1c. Extensions wire into the hook — `pi-coding-agent/src/core/agent-session.ts:540-565`

```ts
this.agent.setBeforeToolCall(async ({ toolCall, args }) => {
    await this._agentEventQueue;  // settle prior events
    if (!this._extensionRunner?.hasHandlers("tool_call")) return undefined;
    try {
        const callResult = await this._extensionRunner.emitToolCall({
            type: "tool_call",
            toolName: toolCall.name,
            toolCallId: toolCall.id,
            input: args as Record<string, unknown>,
        });
        if (callResult?.block) {
            return { block: true, reason: callResult.reason || "Tool execution was blocked by an extension" };
        }
    } catch (err) {
        return { block: true, reason: err instanceof Error ? err.message : `Extension failed, blocking execution: ${String(err)}` };
    }
    return undefined;
});
```

Loop calls `beforeToolCall` → coding-agent broadcasts a `tool_call` event to all loaded extensions → first extension that returns `{block}` wins. Exceptions bubble up as blocks (fail-closed).

### 1d. Concrete blocking extension — `pi-coding-agent/src/core/tools/bash-interceptor.ts:82-95`

Pattern-based block, no UI:

```ts
return {
    check(command: string, availableTools: string[]): InterceptionResult {
        const trimmed = command.trim();
        for (const { regex, rule } of compiled) {
            if (regex.test(trimmed) && availableTools.includes(rule.tool)) {
                return {
                    block: true,
                    message: `Blocked: ${rule.message}\n\nOriginal command: ${command}`,
                    suggestedTool: rule.tool,
                };
            }
        }
        return { block: false };
    },
};
```

### 1e. What pi-mono does **not** do

I grepped the entire installed pi-mono tree for `ui.confirm` and `confirm(` — no inline human prompt is wired into the loop. The TUI talks to the agent **outside** the tool dispatch path. If a vendor wants HITL approval, they implement an extension whose `tool_call` handler awaits whatever UI mechanism it owns (modal, websocket round-trip, Slack thread) and returns `{block, reason}`. The loop never knows.

**Takeaway for hivecore:** the cleanest seam is exactly this — make approval an opt-in `ToolHook` that returns a block decision. UX lives in Layer 3.

---

## 2. OpenAI Codex — explicit `ExecApprovalRequest` event + persistent decision cache

Codex's shape is heavier than pi-mono: the loop emits a structured event into a protocol stream, the host (TUI / VS Code / ACP) renders a chooser, and a `ReviewDecision` flows back. There are *two* call sites — shell exec (via `safety.rs`) and MCP tool calls (via `mcp_tool_call.rs`) — and a guardian-LLM auto-review path on top of either.

### 2a. The protocol event — `codex-rs/core/src/approvals.rs:212-244`

```rust
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExecApprovalRequestEvent {
    /// Identifier for the associated command execution item.
    pub call_id: String,
    /// Identifier for this specific approval callback.
    /// When absent, the approval is for the command item itself (`call_id`).
    /// This is present for subcommand approvals (via execve intercept).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    /// Turn ID that this command belongs to.
    #[serde(default)]
    pub turn_id: String,
    /// The command to be executed.
    pub command: Vec<String>,
    /// The command's working directory.
    pub cwd: AbsolutePathBuf,
    /// Optional human-readable reason for the approval (e.g. retry without sandbox).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional network context for a blocked request that can be approved.
    pub network_approval_context: Option<NetworkApprovalContext>,
    /// Proposed execpolicy amendment that can be applied to allow future runs.
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    /// Proposed network policy amendments (for example allow/deny this host in future).
    pub proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
}
```

The event is wrapped as `EventMsg::ExecApprovalRequest(...)` (`codex-rs/core/src/protocol.rs:1414`):

```rust
pub use crate::approvals::ExecApprovalRequestEvent;
// ...
ExecApprovalRequest(ExecApprovalRequestEvent),
```

`ReviewDecision` (the response) is one of `Approved`, `ApprovedForSession`, `Denied`, `Abort`, plus richer variants like `ApprovedExecpolicyAmendment` and `NetworkPolicyAmendment` that mutate policy as a side effect of the approval.

### 2b. Where the approval decision is computed — `codex-rs/core/src/safety.rs:21-31, 86-115`

`safety.rs` is a pure decision function — does not emit, just classifies:

```rust
#[derive(Debug, PartialEq)]
pub enum SafetyCheck {
    AutoApprove {
        sandbox_type: SandboxType,
        user_explicitly_approved: bool,
    },
    AskUser,
    Reject {
        reason: String,
    },
}
```

Then per-policy:

```rust
match get_platform_sandbox(windows_sandbox_level != WindowsSandboxLevel::Disabled) {
    Some(sandbox_type) => SafetyCheck::AutoApprove {
        sandbox_type,
        user_explicitly_approved: false,
    },
    None => {
        if rejects_sandbox_approval {
            SafetyCheck::Reject { reason: patch_rejection_reason(...).to_string() }
        } else {
            SafetyCheck::AskUser
        }
    }
}
```

`SafetyCheck::AskUser` is the "publish `ExecApprovalRequest`, suspend turn, await response" path; `AutoApprove` and `Reject` short-circuit. Each tool's wrapper turns the enum into either a sandbox-wrapped exec, a synthetic error tool result, or an event emission + await.

### 2c. MCP approval path — `codex-rs/core/src/mcp_tool_call.rs:948-1058`

Three layers stacked:

```rust
async fn maybe_request_mcp_tool_approval(
    sess: &Arc<Session>, turn_context: &Arc<TurnContext>,
    call_id: &str, invocation: &McpInvocation,
    hook_tool_name: &str, metadata: Option<&McpToolApprovalMetadata>,
    approval_mode: AppToolApproval,
) -> Option<McpToolApprovalDecision> {
    // 1. policy fast-path
    if mcp_permission_prompt_is_auto_approved(...) { return None; }
    let approval_required = requires_mcp_tool_approval(annotations);
    if !approval_required && approval_mode != AppToolApproval::Prompt { return None; }

    // 2. session-cached "approved for session" decision
    let session_approval_key = session_mcp_tool_approval_key(invocation, metadata, approval_mode);
    if let Some(key) = session_approval_key.as_ref()
        && mcp_tool_approval_is_remembered(sess, key).await
    {
        return Some(McpToolApprovalDecision::Accept);
    }

    // 3. extension/hook layer (programmatic block, pi-mono shape)
    match run_permission_request_hooks(sess, turn_context, call_id,
        PermissionRequestPayload {
            tool_name: HookToolName::new(hook_tool_name),
            tool_input: invocation.arguments.clone()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
        }).await
    {
        Some(PermissionRequestDecision::Allow) => return Some(McpToolApprovalDecision::Accept),
        Some(PermissionRequestDecision::Deny { message }) =>
            return Some(McpToolApprovalDecision::Decline { message: Some(message) }),
        None => {}
    }

    // 4. guardian-LLM auto-review (optional)
    if routes_approval_to_guardian(turn_context) {
        let decision = review_approval_request(sess, turn_context, review_id.clone(), ...).await;
        // ... apply + return
    }

    // 5. human prompt — render question, send via MCP elicitation, await
    let decision = parse_mcp_tool_approval_elicitation_response(
        sess.request_mcp_server_elicitation(turn_context.as_ref(), request_id, params).await,
        &question_id,
    );
    apply_mcp_tool_approval_decision(sess, turn_context, &decision,
        session_approval_key, persistent_approval_key).await;
}
```

The dispatch site that calls this (`codex-rs/core/src/mcp_tool_call.rs:196-225`):

```rust
if let Some(decision) = maybe_request_mcp_tool_approval(
    &sess, turn_context, &call_id, &invocation,
    &hook_tool_name, metadata.as_ref(), approval_mode,
).await {
    let result = match decision {
        McpToolApprovalDecision::Accept
        | McpToolApprovalDecision::AcceptForSession
        | McpToolApprovalDecision::AcceptAndRemember => {
            return handle_approved_mcp_tool_call(
                sess.as_ref(), turn_context.as_ref(), &call_id, invocation,
                metadata.as_ref(), request_meta, mcp_app_resource_uri,
            ).await;
        }
        // Decline → synthesize error tool result, do not call the MCP server.
        ...
    };
}
```

### 2d. Codex vs pi-mono — what's different

| Concern | pi-mono | Codex |
|---|---|---|
| Loop seam | one hook (`beforeToolCall`) | two seams: `safety::SafetyCheck::AskUser` (exec) + `maybe_request_mcp_tool_approval` (MCP) |
| Decision shape | `{block, reason}` | `ReviewDecision` enum with policy-amendment variants |
| Persistence | none in loop | session cache + on-disk persistent cache, both keyed |
| Guardian / auto-review | none | optional LLM judge runs before human prompt |
| Reason text | empty by default | required, drives UI copy |
| Approval scope | per-call | per-call / per-session / per-amendment |

Codex's `run_permission_request_hooks` step is the pi-mono shape grafted onto a richer system. Hivecore can land pi-mono's shape on day one and grow toward Codex's shape via the same trait.

---

## 3. Hivecore implementation playbook

### 3a. The seam already exists — `crates/hivecore-agent-loop/src/driver.rs:337-350`

```rust
// Pre-hooks. First non-Pass outcome wins.
let pre = self.run_pre_hooks(turn_id, &invocation).await;
let outcome = match pre {
    HookDecision::Pass => {
        self.execute_tool(turn_id, &invocation, signal.clone()).await?
    }
    HookDecision::Override(o) => o,
    HookDecision::FailedContinue(reason) => ToolOutcome::error(reason),
    HookDecision::FailedAbort(reason) => return Err(LoopError::HookAborted(reason)),
    HookDecision::ManualAttention(reason) => {
        return Err(LoopError::ManualAttention(reason))
    }
};
```

`run_pre_hooks` (driver.rs:380-400) iterates `self.hooks: Vec<Arc<dyn ToolHook>>` and pattern-matches `HookOutcome::{Pass, Override, FailedContinue, FailedAbort, ManualAttention}`. **No driver patch is required to land HITL.** A new `ApprovalHook` simply implements `ToolHook` and returns:

| User decision | `HookOutcome` | Effect |
|---|---|---|
| Approve once / for session | `Pass` | tool executes |
| Deny | `FailedContinue { reason }` | loop emits error `ToolResult`; model sees it next turn (pi-mono shape) |
| Abort | `ManualAttention { reason }` | bubbles `LoopError::ManualAttention` to harness |

This is the same shape pi-mono uses (`{block:true, reason}` ≡ `FailedContinue { reason }`), with the bonus that hivecore can also `Override` (auto-approve with a synthetic result) for replay/dry-run, matching Codex's `AcceptAndRemember`.

### 3b. The hook itself — proposed `crates/hivecore-approval-policy/`

Layer 3 crate, depends on `runtime-core` traits only.

```
crates/hivecore-approval-policy/
├── Cargo.toml
├── AGENTS.md
└── src/
    ├── lib.rs            # re-exports
    ├── policy.rs         # ApprovalPolicy enum: Untrusted | OnFailure | OnRequest | Never
    ├── decision.rs       # Decision enum + ApprovalRequest struct
    ├── matcher.rs        # trait ToolMatcher; ToolNameMatcher (deny-list); GlobMatcher (later)
    ├── cache.rs          # in-memory session cache; on-disk persistent cache (later)
    ├── hook.rs           # ApprovalHook: impl ToolHook
    └── prompter.rs       # trait Prompter
```

Key types:

```rust
// policy.rs
pub enum ApprovalPolicy { Untrusted, OnFailure, OnRequest, Never }

// decision.rs
pub enum Decision { Approve, ApproveSession, Deny, Abort }

pub struct ApprovalRequest<'a> {
    pub tool_name: &'a str,
    pub input: &'a serde_json::Value,
    pub reason: Option<&'a str>,    // Codex-style "why this needs approval"
}

// prompter.rs
#[async_trait::async_trait]
pub trait Prompter: Send + Sync {
    async fn ask(&self, req: ApprovalRequest<'_>) -> Decision;
}

// matcher.rs
pub trait ToolMatcher: Send + Sync {
    fn requires_approval(&self, inv: &ToolInvocation) -> Option<&str>; // returns reason if yes
}

// hook.rs
pub struct ApprovalHook {
    policy: ApprovalPolicy,
    prompter: Arc<dyn Prompter>,
    matcher: Arc<dyn ToolMatcher>,
    cache: parking_lot::Mutex<HashSet<(String, u64)>>,  // (tool_name, arg_hash)
}

#[async_trait::async_trait]
impl ToolHook for ApprovalHook {
    async fn before(&self, ctx: ToolHookContext<'_>) -> HookOutcome {
        let Some(reason) = self.matcher.requires_approval(ctx.invocation)
            else { return HookOutcome::Pass; };
        let key = (ctx.invocation.name.to_string(), arg_hash(&ctx.invocation.input));
        if self.cache.lock().contains(&key) { return HookOutcome::Pass; }

        let decision = self.prompter.ask(ApprovalRequest {
            tool_name: &ctx.invocation.name,
            input: &ctx.invocation.input,
            reason: Some(reason),
        }).await;
        match decision {
            Decision::Approve        => HookOutcome::Pass,
            Decision::ApproveSession => { self.cache.lock().insert(key); HookOutcome::Pass }
            Decision::Deny           => HookOutcome::FailedContinue {
                reason: format!("User denied execution of {}", ctx.invocation.name),
            },
            Decision::Abort          => HookOutcome::ManualAttention {
                reason: format!("User aborted at tool {}", ctx.invocation.name),
            },
        }
    }
    async fn after(&self, _: ToolPostContext<'_>) -> PostHookOutcome { PostHookOutcome::Pass }
}
```

Wiring: `agent_loop.add_hook(Arc::new(ApprovalHook::new(policy, prompter, matcher)));`

### 3c. CLI prompter for `hivecore-coder` — `crates/hivecore-coder/src/cli_prompter.rs`

```rust
use hivecore_approval_policy::{ApprovalRequest, Decision, Prompter};
use std::io::{self, BufRead, Write};
use tokio::task;

pub struct CliPrompter;

#[async_trait::async_trait]
impl Prompter for CliPrompter {
    async fn ask(&self, req: ApprovalRequest<'_>) -> Decision {
        let tool = req.tool_name.to_string();
        let input = serde_json::to_string_pretty(req.input).unwrap_or_default();
        let reason = req.reason.unwrap_or("(no reason given)").to_string();

        // stdin is blocking; hop to a blocking thread so we don't stall the runtime.
        task::spawn_blocking(move || {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let _ = writeln!(out, "\n── approval required ──");
            let _ = writeln!(out, "tool:   {}", tool);
            let _ = writeln!(out, "reason: {}", reason);
            let _ = writeln!(out, "input:\n{}", input);
            let _ = writeln!(out, "[a]llow once  [s]ession allow  [d]eny  [q]uit");
            let _ = write!(out, "> ");
            let _ = out.flush();

            let stdin = io::stdin();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() { return Decision::Abort; }
            match line.trim().chars().next().unwrap_or('d') {
                'a' | 'A' | 'y' | 'Y' => Decision::Approve,
                's' | 'S'             => Decision::ApproveSession,
                'q' | 'Q' | '\u{4}'   => Decision::Abort,
                _                     => Decision::Deny,
            }
        }).await.unwrap_or(Decision::Abort)
    }
}
```

### 3d. ACP prompter stub — `crates/hivecore-acp-server/src/approval_bridge.rs`

ACP defines `session/request_permission` (server → client; client returns user choice). This `Prompter` impl bridges hook ↔ ACP RPC via a oneshot channel; the existing ACP server task owns the JSON-RPC writer and drains pending approvals.

```rust
use hivecore_approval_policy::{ApprovalRequest, Decision, Prompter};
use tokio::sync::{mpsc, oneshot};

pub struct AcpPrompter {
    tx: mpsc::Sender<PendingApproval>,
}

pub struct PendingApproval {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub reason: Option<String>,
    pub respond: oneshot::Sender<Decision>,
}

#[async_trait::async_trait]
impl Prompter for AcpPrompter {
    async fn ask(&self, req: ApprovalRequest<'_>) -> Decision {
        let (respond, rx) = oneshot::channel();
        let pending = PendingApproval {
            tool_name: req.tool_name.to_string(),
            input: req.input.clone(),
            reason: req.reason.map(str::to_string),
            respond,
        };
        if self.tx.send(pending).await.is_err() { return Decision::Abort; }
        // ACP server task picks it up, sends `session/request_permission`,
        // awaits client reply, maps allow_once|allow_always|reject_once|reject_always
        // → Decision, fires the oneshot.
        rx.await.unwrap_or(Decision::Abort)
    }
}
```

### 3e. Wiring summary

```text
hivecore-coder main:
  ├─ build AgentLoop
  ├─ approval_policy = ApprovalPolicy::Untrusted        // from Agent TOML
  ├─ prompter = Arc::new(CliPrompter)                   // (or AcpPrompter for hivecore-acp-server)
  ├─ matcher = Arc::new(ToolNameMatcher::deny_list(
  │       &[("bash", "shell exec"),
  │         ("write_file", "filesystem write"),
  │         ("edit_file", "filesystem edit")]))
  └─ loop.add_hook(Arc::new(ApprovalHook::new(policy, prompter, matcher)))
```

No changes needed in `runtime-core`, `agent-loop`, or any tool crate. Approval lands as a Layer 3 bolt-on, exactly the way MCP and skills landed.

---

## 4. Open questions / parking lot

- **MCP tool approval.** When `mcp_call` (the meta-tool) dispatches an MCP tool, approval should consider the *underlying* MCP tool name + server, not the meta-tool name. Cleanest: have `ApprovalHook` peek into `mcp_call` invocations and synthesize a virtual `ToolInvocation { name: "mcp__<server>__<tool>", input: <inner-args> }` for matching. Codex effectively does this — its MCP path is a sibling call site, not the same hook. ADR territory if we want to mirror.
- **Persistent approvals.** Codex's `ApprovedForSession` is in-process; `AcceptAndRemember` writes to disk. ACP can offer "always" → wants on-disk store keyed by `(tenant, agent, tool, arg_template_hash)`. Defer to v0.2 of the crate; in-memory cache lands in v0.1.
- **Reason text.** pi-mono and the current hook trait don't pass a "why is this risky" string into `before()`. Codex requires it. Solution above keeps the reason in the `ToolMatcher` (per-tool static reason), with room to grow toward Codex-style dynamic reasons (e.g., "command tries to write outside cwd").
- **ReviewDecision::ApprovedExecpolicyAmendment.** Codex lets a single approval mutate the live policy ("allow this prefix forever"). Maps to a fifth `Decision::ApproveAndExtend(PolicyAmendment)` variant + a way for `ApprovalHook` to mutate `matcher` in place. Defer.
- **Guardian / auto-review.** Codex routes some approvals through an LLM judge before bothering the human. In hivecore this would be a *second* `ToolHook` registered before `ApprovalHook` that returns `Override` on auto-approve or `Pass` on uncertain. Already supported by the existing trait.
