# crates/hivecore-runtime-core/AGENTS.md

**Layer 1 — I/O-free trait surface.** This crate is the substrate. Nothing here may touch the network, filesystem, processes, or syscalls.

## What lives here

| Module | Trait / Type | Purpose |
|---|---|---|
| `tool` | `Tool` | Async tool contract. Models call these. |
| `hook` | `ToolHook` + `HookOutcome` | Pre/post tool call gate. Codex trichotomy + `ManualAttention`. |
| `lifecycle` | `LifecycleHook` + `LifecycleEvent` | Pre/post turn/agent/message + pre-model-request gate. |
| `model` | `ModelAdapter`, `ModelRequest`, `ModelChunk` | LLM provider contract. |
| `message` | `AgentMessage`, `ContentBlock`, `StopReason` | Conversation primitives. |
| `event` | `AgentEvent` | Lifecycle telemetry (observer-only — see `LifecycleHook` for gate). |
| `sink` | `EventSink`, `NoopSink`, `VecSink`, `FanOutSink` | Event subscriber trait + helpers. |
| `state` | `AgentState`, `ThinkingLevel`, `ToolDescriptor` | Snapshot + serialisation contract. |
| `transform` | `ContextTransform` | Prompt rewriting / steering / follow-up injection. |
| `abort` | `AbortSignal`, `AbortHandle` | Cooperative cancellation. |
| `error` | `RuntimeError`, `RuntimeResult` | `thiserror`-shaped library errors. |
| `ids` | `SessionId`, `TurnId`, `MessageId`, `ToolCallId` | Strongly-typed newtype IDs. |

## Hard rule

**No I/O imports.** No `tokio::fs`, no `reqwest`, no `std::fs`, no `Command`, no `tracing-subscriber` setup. `tracing` macros are fine; concrete subscribers belong upstream.

## Test commands

```
cargo test -p hivecore-runtime-core
cargo clippy -p hivecore-runtime-core --all-targets -- -D warnings
```

## v0.2 backlog

- Rename package: `runtime-core-spike` → `hivecore-runtime-core`.
- Relocate to `crates/hivecore-runtime-core/` (preserve commit history via `git mv`).
- Promote `VecLifecycleHook` (currently in test module) to a public test-support feature.
