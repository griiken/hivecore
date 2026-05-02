# crates/hivecore-coder/AGENTS.md

**Sample harness — proves Layer 1+2 substrate composability.**

Different shape than `hivecore-acp-server`: no ACP wire, no skills, no agent
registry, no config TOML. Direct CLI loop showing the minimum needed to
compose a working coding agent from substrate primitives.

## Why this crate exists

ADR-016 says Layer 3 is forkable per org. To make that claim load-bearing we
need *more than one* Layer 3 in tree: if both shapes drop into the same
substrate without any substrate change, the substrate property is real.

`hivecore-acp-server` was the first Layer 3. `hivecore-coder` is the second —
deliberately different in shape:

| Axis | `hivecore-acp-server` | `hivecore-coder` |
|---|---|---|
| Wire | ACP JSON-RPC over stdio | argv prompt → stdout text |
| Config | TOML agent registry, skills dir | env vars only |
| Sink | Marshals to ACP `session/update` | Pretty-prints to stderr |
| Hooks | None (yet) | `TurnBudgetHook` (Gate-plane shape) |
| Tools | builtins + skill-tools | builtins + custom `ClockTool` |
| LOC | ~700 | ~120 (binary) |

## What lives here

| Module | Purpose |
|---|---|
| `bin/main.rs` | CLI entry: parse args/env → wire substrate → run → print final text. |
| `turn_budget` | `LifecycleHook` aborting after N model calls. Gate-plane analog. |
| `clock_tool` | Custom `Tool` (returns UTC time). Demonstrates `Tool` trait extension. |
| `stderr_sink` | `EventSink` that pretty-prints lifecycle events. |

## Run

```
export OPENAI_API_KEY=sk-...
cargo run -p hivecore-coder -- "list rust files under crates/"
```

Env knobs:
- `HIVECORE_MODEL` — default `gpt-5.4-nano`.
- `HIVECORE_TURN_BUDGET` — default `16`. `TurnBudgetHook` aborts past this.
- `HIVECORE_WORKSPACE` — default `$PWD`. Bounds `WorkspaceRoot::resolve`.

## Test commands

```
cargo test -p hivecore-coder
cargo clippy -p hivecore-coder --all-targets -- -D warnings
```

## v0.2 backlog

- **Phase A (ADR-026) — session resume.** Default-wire `hivecore-persistence::SessionWriter` as one of the sinks; add `--continue / --session <id|name> / --name <handle>` CLI flags; `SessionReader::messages()` → `AgentLoopBuilder::resume(...)` round-trip; advisory lock-file at `<session_id>.jsonl.lock`.
- **Phase B (ADR-026) — compaction.** Wire default `hivecore-compaction::SummarizingTransform` + `TokenThresholdHook` + `EmergencyDropHook`. Per-agent override under `[agent.compaction]` TOML.
- `--system-prompt-file` to load Codex-default base instructions from
  `hivecore-system-prompt` (currently uses an inline minimal prompt).
- Multi-turn REPL mode (read prompts from stdin in a loop).
- Cancel-on-Ctrl-C wired through `AbortSignal::new()`.
