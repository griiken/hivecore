# crates/hivecore-mcp-client — AGENTS.md

Layer-3 opt-in MCP client. Implements ADR-027.

## Status (v0.2 LANDED — lazy 3-meta-tool default)

Shipped:
- `rmcp = "0.8"` wired; stdio transport via `TokioChildProcess`. Features pinned per ADR-027 §9 (incl. `server` for upstream mis-gate).
- `client::McpClient` — config + per-(tenant, server) connection cache. `get_or_connect(server)` opens a child process the first time, reuses cached handle thereafter.
- `client::call_raw_tool` — applies allow/deny → dispatches `peer().call_tool(...)` with per-server `tool_timeout_sec`.
- `client::list_raw_tools` — `peer().list_tools(Default::default())` for discovery.
- `${env:VAR}` resolution at connect time. Unset var → `Config` error before spawn.
- Three meta-tools impl `hivecore_runtime_core::Tool` — `McpServersTool` / `McpDiscoverTool` / `McpCallTool`. `default_meta_tools(client)` returns the trio.
- 20 unit tests across `name`, `config`, `client`, `meta_tools`.

## Surface

- `name::qualify_tool_name(server, tool)` — Codex `mcp__<server>__<tool>` namespace, 64-byte cap, SHA-1 12-hex suffix on overflow / collision.
- `config::McpUserConfig` — TOML schema. Two layered files: `~/.hivecore/mcp.toml` (user) + `<workspace>/.hivecore/mcp.toml` (workspace overrides). `merge` does per-key replace.
- `config::ApprovalMode` — `Always | Never | OnRequest`. `OnRequest` is the default; persistence at `(tenant, server, tool)` granularity is owned by the audit plane (ADR-019) — not yet wired.
- `error::McpError` — typed errors. `DeniedByDenylist` and `DisabledByAllowlist` are distinct so audit can tell which gate fired.
- `meta_tools::{McpServersTool, McpDiscoverTool, McpCallTool, default_meta_tools}`.

## v0.2.x backlog (this crate)

- [ ] Eager-mode builder (`McpClient::eager_tools(cfg).await -> Vec<Arc<dyn Tool>>`) — Codex shape, per ADR-027 §5.
- [ ] `tools/list_changed` invalidates the discovery cache and re-lists. (Codex just logs — we fix.)
- [ ] 2 KiB description cap with truncation marker. (Codex has none — we fix.)
- [ ] First-run import from `~/.codex/config.toml` and `~/.claude.json`.
- [ ] Custom `ClientHandler` — `create_message` returns `method_not_found` (sampling deferred); `list_roots` reads tenancy plane; `create_elicitation` bridges to ACP elicitation.
- [ ] Tool result truncation at 1024 bytes (Codex parity).
- [ ] HTTP / SSE / WS transports (currently `ConnectFailed` with reason).
- [ ] Per-(tenant, server, tool) approval cache durable in audit JSONL (currently config-only).
- [ ] Default-wire in `hivecore-coder` and `hivecore-acp-server` with `--no-mcp` flag.

## Out of scope (this crate)

- Outbound MCP server (`hivecore-mcp-server` binary) — separate crate, separate ADR (v0.3+).
- Sampling — deferred to v0.3+ behind a written security review (server can spend user's tokens on server-supplied prompts).

## Tests

`cargo test -p hivecore-mcp-client` — pure-logic unit tests for `name` + `config`. No I/O. rmcp-shaped integration tests land with the rmcp wiring.
