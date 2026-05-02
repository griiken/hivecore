# crates/hivecore-agent-loop/AGENTS.md

**Layer 2 — the loop driver.** Pi-shape nested loops with hook gates between every step.

## What lives here

| Module | Purpose |
|---|---|
| `driver` | `AgentLoop` + `AgentLoopBuilder`. The main run loop. |
| `accumulator` | Folds `ModelStream` into one `AgentMessage::Assistant` + extracts tool calls. |
| `registry` | `ToolRegistry` (`Arc<HashMap<String, Arc<dyn Tool>>>`). |
| `sink` | Re-exports of `hivecore_runtime_core::{EventSink, NoopSink, VecSink, FanOutSink}`. |
| `steering` | `SteeringSource` trait (`drain_steering` / `drain_follow_ups`). |
| `spawn` | `SpawnAgentTool` + `SubAgentSpec` + `ClosureSpec` — sub-agent recursion as a Tool. |
| `error` | `LoopError` (`thiserror`). |

## Load-bearing invariants

(see top of `driver.rs` for the canonical statement)

1. **Append-only message log.** Never mutate a prior message. Codex prefix-preservation invariant.
2. **Insert-on-change.** Mid-session env updates arrive as new `AgentMessage::Custom`, not edits.
3. **Compaction belongs in `ContextTransform::transform_outgoing`.** Trait slot reserved.

## Lifecycle hook firing order

Per `AgentLoop::run`:

```
AgentStart
  PostMessageCommit (user prompt)
  loop:
    PreTurn
    PreModelRequest
    PostMessageCommit (assistant)
    PostTurn
AgentEnd
```

`FailedAbort` short-circuits with `LoopError::HookAborted`. `ManualAttention` short-circuits with `LoopError::ManualAttention`.

## Test commands

```
cargo test -p hivecore-agent-loop
OPENAI_API_KEY=... cargo run --example openai_agent
```

## v0.2 backlog

- **Wire `transform_outgoing` into the model-call site (ADR-026).** Trait slot reserved but not currently called. Drop in: `req.messages = ctx_transform.transform_outgoing(&state, req.messages).await?` before each `model.complete(req)`.
- **`AgentLoopBuilder::resume(messages: Vec<AgentMessage>)` (ADR-026).** Seeds `state.messages` so first model call reflects prior history. Required for `hivecore-coder --continue / --session`.
- **`AgentLoopBuilder::context_transform(Arc<dyn ContextTransform>)`.** Currently no setter — add.
- **Mid-turn `ContextWindowExceeded` retry (ADR-026).** Adapter returns typed error → driver inner-loop catches → fires `EmergencyDropHook` → `messages.remove(0)` + retry, bounded.
- Streaming sub-agent events back to the parent's sink as `ToolExecUpdate` payloads (Codex's `forward_events`).
- `MAX_SUBAGENT_DEPTH` enforcement.
