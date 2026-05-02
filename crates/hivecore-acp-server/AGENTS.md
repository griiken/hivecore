# crates/hivecore-acp-server/AGENTS.md

**Layer 3 — ACP/JSON-RPC server.** Binary `hivecore-acp`. Editors (Zed, etc.) spawn this as a subprocess; it speaks Agent Client Protocol over stdio.

## What lives here

| File | Purpose |
|---|---|
| `src/server.rs` | `ServerConfig` (env-driven), `ServerState` (sessions + agents + skills), `PersonaRouter` (skill `agent:` → sub-agent). |
| `src/bridge.rs` | `AcpEventSink` — translates `hivecore_runtime_core::AgentEvent` → ACP `SessionUpdate` notifications. |
| `src/bin/main.rs` | Binary entry point. Builds the `Agent.builder()` from the official `agent-client-protocol` SDK. |

## Env-driven config

| Var | Purpose | Default |
|---|---|---|
| `OPENAI_API_KEY` | Required. | — |
| `HIVECORE_WORKSPACE` | Workspace root the agent operates in. | `cwd` |
| `HIVECORE_AGENT_DIR` | Optional dir of `*.toml` agent definitions to layer atop builtins. | (none) |
| `HIVECORE_AGENT` | Default agent id. | `default` |
| `HIVECORE_SKILL_DIR` | Optional dir of `*.md` skill files. | (none) |
| `HIVECORE_MODEL` | Override agent's model id (rarely needed). | (agent's value) |

## ACP wire shape

Standard JSON-RPC 2.0 over stdio (no Content-Length framing). Methods we handle:
- `initialize` → returns capabilities + protocol version.
- `session/new` → returns a fresh `SessionId`.
- `session/prompt` → drives the agent, streams `session/update` notifications, responds with `stopReason`.

Notifications we send (per ACP `SessionUpdate` enum):
- `agent_message_chunk` (text streaming)
- `agent_thought_chunk` (`reasoning_content` from gpt-5/o-series)
- `tool_call` (start)
- `tool_call_update` (status: `Completed` | `Failed` + `raw_output`)

## Test commands

```
cargo build -p hivecore-acp-server --release
# Drive it with a JSON-RPC handshake — see CHANGELOG / commit log for the
# canonical smoke flow.
```

## v0.2 backlog

- `session/cancel` notification handling (currently we abort the whole run; should scope to one prompt).
- `session/load` (resume from a sessions JSONL via `persistence-spike`).
- `session/request_permission` (currently auto-allows everything; should ask the client for risky tool calls).
- MCP server forwarding (the SDK supports it; we just don't wire it yet).
