# MCP integration — full lifecycle prior-art

Research date: 2026-05-01. Targets: Codex (`openai/codex` Rust workspace),
Claude Code (closed binary, public docs at `docs.claude.com/en/docs/claude-code/mcp`),
jcode (`1jehuang/jcode` Rust), pi-mono (`badlogic/pi-mono` TS), and the
official rmcp Rust SDK (`modelcontextprotocol/rust-sdk`).

This file is for the hivecore team to read before writing integration code.
It is observation-only, not a design doc.

---

## Spec recap

MCP is a JSON-RPC 2.0 protocol over a duplex byte stream (stdio child process,
streamable HTTP, SSE, WebSocket). Lifecycle: `initialize` (client → server,
declares `clientInfo` + `capabilities` + protocol version) → server replies
with `serverInfo` + `capabilities` + optional `instructions` → client sends
`notifications/initialized` → operations → graceful shutdown by closing the
transport. There is no explicit `shutdown` RPC; the client just stops the
transport.

Three primitives sit on top of the lifecycle: **tools** (model-callable
functions: `tools/list`, `tools/call`), **resources** (URI-addressable read-only
content: `resources/list`, `resources/read`, `resources/templates/list`,
`resources/subscribe` + `notifications/resources/updated`), and **prompts**
(parameterized prompt templates: `prompts/list`, `prompts/get`). Servers can
also declare `logging` (`logging/setLevel` + `notifications/message`),
`completion/complete` for argument auto-complete, and capability flags such as
`tools.listChanged` / `resources.listChanged` / `prompts.listChanged` that
gate the matching `notifications/.../list_changed` push from server to client.

Two reverse-direction features exist where the **server** issues requests back
to the **client**: **sampling** (`sampling/createMessage` — server asks the
client's LLM for a completion; security-sensitive because the client's model
+ tokens get used on server-supplied prompts), **elicitation**
(`elicitation/create` — server asks the client to collect structured user
input mid-tool-call; added later in the spec), and **roots** (`roots/list` —
server queries client for filesystem roots it should consider in scope). All
three are gated by capabilities the client declares in `initialize`. Progress
streaming uses `notifications/progress` keyed off a `progressToken`.
Cancellation uses `notifications/cancelled` keyed off a request ID.

---

## rmcp Rust SDK (the SDK we'd use)

Repo: `modelcontextprotocol/rust-sdk`, crate `crates/rmcp`. Description:
"Rust SDK for Model Context Protocol". 3.3k★. Active.

### Architecture

`rmcp` is one crate with two roles (`RoleClient`, `RoleServer`) gated on the
`client` / `server` Cargo features. The same `Service<R>` trait is implemented
once per role; `ServiceExt::serve(transport)` performs `initialize`, returns a
`RunningService<R, S>` whose `peer()` exposes typed RPC methods. Transports
are pluggable via the `IntoTransport` trait. `Cargo.toml` keeps every
transport behind a feature so a thin client never compiles wasmtime/reqwest.

Public re-exports in `crates/rmcp/src/lib.rs`:

```
pub use service::{Peer, Service, ServiceError, ServiceExt};
pub use service::{RoleClient, serve_client};       // when feature = "client"
pub use service::{RoleServer, serve_server};       // when feature = "server"
pub use handler::client::ClientHandler;            // client-side callbacks
pub use handler::server::ServerHandler;            // server-side callbacks
pub mod model;          // every wire type
pub mod transport;      // transports
pub mod task_manager;   // server-side request fanout
```

### Public surface

`crates/rmcp/src/service/client.rs`:

- `pub async fn serve_client<S, T, E, A>(service: S, transport: T) -> Result<RunningService<RoleClient, S>, ClientInitializeError>`
- `pub type ServerSink = Peer<RoleClient>` — the typed remote-end handle.
- `ServiceExt::serve_with_ct(self, transport, CancellationToken)` for graceful shutdown.
- `ClientInitializeError` enumerates `ExpectedInitResponse`, `ConflictInitResponseId`, `ConnectionClosed`, `TransportError`, `JsonRpcError`, `Cancelled`.

`Peer<RoleClient>` exposes the typed client RPCs (paraphrased — the methods
the calling code uses): `list_tools(params)`, `call_tool(params)`,
`list_resources(params)`, `read_resource(params)`, `list_resource_templates(params)`,
`subscribe(params)`, `unsubscribe(params)`, `list_prompts(params)`,
`get_prompt(params)`, `set_level(params)`, `complete(params)`,
`send_cancelled(params)`, `send_progress(params)`, `send_roots_list_changed()`.

### ClientHandler trait (server-initiated callbacks)

`crates/rmcp/src/handler/client.rs` defines the trait the embedder implements
to handle requests/notifications **from** the server:

Requests (return values):
- `ping` — health.
- `create_message(CreateMessageRequestParams)` → `CreateMessageResult`. **Sampling.** Default impl returns `method_not_found`. Implementor must opt in.
- `list_roots()` → `ListRootsResult`. Default: empty list.
- `create_elicitation(CreateElicitationRequestParams)` → `CreateElicitationResult`. Default: declines all. Two variants in the param: `FormElicitationParam` (schema-based form) and `UrlElicitationParam` (browser flow).
- `on_custom_request` — escape hatch for non-spec methods.

Notifications (no return):
- `on_cancelled`, `on_progress`, `on_logging_message`, `on_resource_updated`, `on_resource_list_changed`, `on_tool_list_changed`, `on_prompt_list_changed`, `on_url_elicitation_notification_complete`, `on_custom_notification`.

`get_info()` returns the `ClientInfo` (capabilities + clientInfo) advertised
in `initialize`. Capabilities the embedder enables here gate what the server
is allowed to ask back for — declaring `sampling = false` blocks
`create_message` at the protocol layer.

### Transports

`crates/rmcp/src/transport/`:

| File | Purpose |
|---|---|
| `child_process.rs` | `TokioChildProcess::new(command).await` — spawns a subprocess, frames `stdin`/`stdout` line-delimited JSON. Feature `transport-child-process`. |
| `streamable_http_client.rs` + `auth.rs` | Streamable HTTP + SSE-formatted responses. Features `transport-streamable-http-client`, `transport-streamable-http-client-reqwest`, `transport-streamable-http-client-unix-socket`. |
| `streamable_http_server.rs` | Server side of streamable HTTP. |
| `ws.rs` | WebSocket via `tokio-tungstenite`. Feature `transport-ws`. |
| `async_rw.rs`, `io.rs`, `sink_stream.rs` | Generic adapters over `AsyncRead+AsyncWrite`, `Stream`+`Sink`, raw stdio. |
| `worker.rs` | Channel-pair transport for in-process mocks/tests. |

### Auth

Feature `auth` pulls in `oauth2 = "5"`. Feature `auth-client-credentials-jwt`
adds `jsonwebtoken` for `private_key_jwt`. The OAuth handshake (RFC 9728
Protected Resource Metadata + RFC 8414 Authorization Server Metadata + RFC 8707
resource indicators) is built in to the streamable-HTTP client; rmcp itself
does **not** persist tokens — that's the embedder's job (Codex stores them
in keychain, Claude Code in keychain on macOS or a credentials file).

### Dep weight

Default deps are light: `tokio`, `serde`, `serde_json`, `futures`, `tracing`,
`async-trait`, `thiserror`, `tokio-util`, `pin-project-lite`. Anything else
(reqwest, jsonwebtoken, tokio-tungstenite, schemars, base64) is optional.
A pure-stdio client needs only `client`, `transport-child-process`,
`transport-io`. That keeps the cold-start dep set small enough for a
hivecore Layer 2 crate.

---

## Codex

Codex is the heaviest reference impl. It splits MCP across **three** crates
and treats MCP as a first-class extension surface that even ships its own
sub-protocol (`codex_apps`).

### Crate map

| Crate | Role |
|---|---|
| `codex-rs/rmcp-client/` | **Low-level MCP client** wrapping the rmcp SDK. Owns `RmcpClient` (per-server runtime), stdio launcher abstraction, OAuth, elicitation plumbing, logging handler. |
| `codex-rs/codex-mcp/` | **Aggregator** layer above `rmcp-client`. `McpConnectionManager` keeps N clients keyed by server name, normalizes tool names, filters allow/deny lists, dispatches calls. |
| `codex-rs/mcp-server/` | **Codex-as-MCP-server.** Codex itself exposes its `codex` agent over MCP so other clients (Zed, Claude Desktop) can mount it. Independent of the two above. |

Direction of dependency: `core → codex-mcp → rmcp-client → rmcp`.
`mcp-server` is a leaf binary depending on `core` + `rmcp`.

### Config schema

`codex-rs/config/src/mcp_types.rs` (`RawMcpServerConfig` / `McpServerConfig`).
Lives in `~/.codex/config.toml` under `[mcp_servers.<name>]`. Both stdio and
streamable-HTTP transports supported in one schema (transport implied by
which fields are present):

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "${env:GITHUB_TOKEN}" }
env_vars = []
cwd = "/path"
enabled = true
supports_parallel_tool_calls = false
startup_timeout_sec = 30
tool_timeout_sec = 60
default_tools_approval_mode = "on-request"   # AppToolApproval enum
enabled_tools = ["create_issue", "list_repos"]   # allowlist
disabled_tools = []                              # denylist
scopes = ["repo"]
oauth_resource = "https://api.github.com"

[mcp_servers.github.tools.create_issue]
approval_mode = "always"   # per-tool override
```

`McpServerTransportConfig` is an enum: `Stdio { ... }` or
`StreamableHttp { url, bearer_token_env_var, ... }`. The CLI lives in
`codex-rs/cli/src/mcp_cmd.rs` with subcommands `list / get / add / remove /
login / logout`. `add` writes into `~/.codex/config.toml` via
`ConfigEditsBuilder`. `login`/`logout` call into `codex-rmcp-client`'s
`perform_oauth_login` / `delete_oauth_tokens`.

### Spawn / handshake

`codex-rs/rmcp-client/src/stdio_server_launcher.rs` defines a sealed
`StdioServerLauncher` trait with two impls: `LocalStdioServerLauncher`
(spawn as a direct child of the orchestrator using
`tokio::process::Command` → wrap in `rmcp::transport::child_process::TokioChildProcess`),
and `ExecutorStdioServerLauncher` (route the spawn through
`codex_exec_server` for sandboxing/process-group control). Both return a
unified `StdioServerTransport` that implements rmcp's `Transport<RoleClient>`
trait. Process placement is hidden from `RmcpClient`.

`codex-rs/rmcp-client/src/rmcp_client.rs` builds two flavours of
`RmcpClient`: `new_stdio_client(...)` and `new_streamable_http_client(...)`.
Internal state machine (`ClientState::Connecting / Initializing / Ready`)
uses a `Mutex<ClientState>` plus a `Semaphore` lock for session-recovery
serialization. Initialize is performed via `Self::connect_pending_transport`
which calls `serve_client_with_ct` and then asserts the response carried
`peer_info()`. On 404-style session-expiry the streamable-HTTP path
re-initializes within the same `RmcpClient` so callers don't observe the
reconnect.

Codex's `ClientInfo` is built from `codex_protocol`. Capabilities advertised:
`elicitation` is enabled (`elicitation_client_service.rs`); `sampling` is
**not** advertised (no `create_message` impl) — Codex never lets a server
call back into its model. `roots` is not advertised in current code.
Server-side `tool_list_changed` is observed via the rmcp default handler;
manager refreshes its cache on receipt.

### Tool exposure / namespacing

`codex-rs/codex-mcp/src/tools.rs`. Codex preserves the **raw** MCP
`server_name` + `tool.name` for routing while sanitizing what the model
sees:

- Model-visible tool name: `mcp__<sanitized_namespace>__<sanitized_name>`,
  must be ≤ 64 bytes, must match the OpenAI Responses API charset (handled
  by `sanitize_responses_api_tool_name`).
- On collision, candidates get suffixed with a SHA-1 hash of their raw
  identity.
- `ToolFilter::from_config(cfg)` builds an allow/deny pair from
  `enabled_tools` (None means no allowlist) + `disabled_tools`. The rule
  is `allows(tool) = enabled.is_none_or(contains(tool)) && !disabled.contains(tool)`.
  Same surface as Claude Code's `enabled` / `disabled` (see below).
- Tools fetched via `list_tools_for_client_uncached`, then fed through
  `qualify_tools` and cached. Cache duration emitted as a metric.

### Permission UX

Per-tool approval is configured in the per-server TOML (`approval_mode` =
`always` / `on-request` / `never`) and per-tool overrides under
`[mcp_servers.<name>.tools.<tool>]`. At call time
`codex-rs/core/src/mcp_tool_call.rs`'s `handle_mcp_tool_call`:

1. Parses arguments JSON.
2. Looks up `lookup_mcp_tool_metadata` for tool annotations.
3. Calls `mcp_permission_prompt_is_auto_approved` against the server's
   permission profile + the connector's `app_too...` check
   (only Codex's own "apps" connector skips approval).
4. If not auto-approved, emits `McpToolCallBegin` + uses the
   guardian/elicitation channel to prompt the user (CLI dialog or app-server
   protocol message).
5. On accept, calls `RmcpClient::call_tool` and emits `McpToolCallEnd`.

Approval policy is tiered: workspace `[mcp_servers.X.default_tools_approval_mode]`
covers everything in that server, per-tool override beats it, and a
runtime `--ask-for-approval` flag tightens the floor. Hooks (`run_permission_request_hooks`)
let extensions intercept before the user prompt.

### Allowlist / denylist

Allow/deny is the per-tool `enabled_tools` / `disabled_tools` shown above.
There is **no** server-name allowlist at this layer — Codex assumes the
admin controls `~/.codex/config.toml`.

### Description budget / deferred loading

`tool_with_model_visible_input_schema` rewrites schemas before exposing them
to the model (e.g. drops OpenAI-internal `file_input` declarations). Each
`ToolInfo` carries `server_instructions` from `serverInfo.instructions`.
Codex caches per-server tool lists (with metric
`codex.mcp.tools.cache_write.duration_ms`); cache invalidation happens on
`notifications/tools/list_changed`. There is no spec-level "deferred load"
analogue to Claude Code's tool search — every cached tool is sent to the
model on every turn.

### Notifications

`codex-rs/rmcp-client/src/logging_client_handler.rs` implements
`on_logging_message` (forwards to `tracing` and to the user-facing
`McpStartupUpdateEvent` channel when relevant) and `on_progress` (routes
to the active call's progress token). `on_resource_list_changed`,
`on_tool_list_changed`, `on_prompt_list_changed` invalidate the manager's
caches.

### Resources & Prompts

Yes, both. `read_mcp_resource` is exported from `codex-mcp/lib.rs`.
`McpConnectionManager` aggregates resources + resource templates across
servers. Prompts: `ListPromptsRequest` / `GetPromptRequest` are wired
through `Peer<RoleClient>`.

### Sampling

**Not implemented.** `ClientHandler::create_message` is left at default
(`method_not_found`). Codex deliberately won't lend its model.

### OAuth

`codex-rs/rmcp-client/src/oauth.rs` + `perform_oauth_login.rs` +
`auth_status.rs`. Tokens stored via the `keyring` crate (per-OS native
keychain on macOS/Windows/Linux/BSD via target-conditional features).
Streamable-HTTP transport gets an OAuth-aware adapter
(`http_client_adapter.rs`); refresh runs before every operation
(`refresh_oauth_if_needed` in `RmcpClient::list_tools`). RFC 9728 Protected
Resource Metadata discovery, fall back to RFC 8414, with retry-without-scopes
for servers that 400 on unknown scopes.

### Reconnect / death

Stdio: process death surfaces via `Transport::receive() -> None` →
`ConnectionClosed`. Manager marks server as failed; restart is manual
(re-running with the same config). Streamable-HTTP: session-expiry 404 is
caught inside `RmcpClient` and a fresh `initialize` is performed in-place
under the recovery `Semaphore`.

### One full call (user prompt → model → MCP tool → result)

1. User starts a turn. `Session` collects per-server `ToolInfo` from
   `McpConnectionManager` (cached or refreshed on `tool_list_changed`).
2. Each `ToolInfo` is converted to a Responses API tool declaration with
   `mcp__<ns>__<name>` plus the model-visible input schema.
3. Model emits a tool call with that qualified name.
4. Codex demangles to `(server_name, raw_tool_name)`.
5. `handle_mcp_tool_call` parses arguments, runs hook + permission checks,
   either auto-approves or emits an elicitation/guardian prompt.
6. On approval, `McpConnectionManager::call_tool(server, tool, args)` →
   `AsyncManagedClient::call_tool` → `RmcpClient::call_tool` →
   `Peer<RoleClient>::call_tool` over the wire.
7. Server replies with `CallToolResult { content, isError, ... }`.
8. Codex emits `McpToolCallEnd`, optional truncation via `TruncationPolicy`,
   feeds the tool result back into the next model turn.

---

## Claude Code

Closed binary; behaviour reconstructed from
`docs.claude.com/en/docs/claude-code/mcp` (fetched 2026-05-01).

### Config

Three file scopes, last-wins precedence:
- **User scope**: `~/.claude.json` (`mcpServers` key) — applies to all sessions.
- **Project scope**: `.mcp.json` at repo root — committed to git, team-wide.
- **Local scope**: `.claude/settings.local.json` — per-checkout overrides.
- **Plugin-provided**: bundled in plugin's `plugin.json` or alongside as
  `.mcp.json`, can use `${CLAUDE_PLUGIN_ROOT}` / `${CLAUDE_PLUGIN_DATA}`
  env-var expansion.
- **Managed**: `managed-mcp.json` deployed to a system path. When present
  it is **exclusive** — users cannot add servers, only the managed list runs.

Schema (`mcpServers.<name>` → one of):
```json
{ "type": "stdio", "command": "...", "args": [...], "env": {...} }
{ "type": "http",  "url": "...", "headers": {...}, "oauth": {...} }
{ "type": "sse",   "url": "...", "headers": {...} }
```

`oauth` sub-object can include `authServerMetadataUrl`,
`scopes`, `clientId`, `clientSecret` (kept in keychain, not file).
Env-var interpolation `${VAR}` is supported in values.

CLI: `claude mcp add / get / list / remove`, `claude mcp add-from-claude-desktop`,
`claude mcp serve` (Claude Code itself acts as an MCP server). Within a
session the slash-command is `/mcp` for status + login + auth-clear.

### Spawn

Stdio: child process started lazily on session start. HTTP/SSE: lazy connect
to URL with bearer/OAuth headers. Plugin MCP servers connect at session
startup; toggling a plugin needs `/reload-plugins` to start/stop its servers.

### Handshake

Capability advertisement is opaque (binary). Observed: client supports
`tool_list_changed` and processes `list_changed` for tools+resources+prompts
(docs: "Dynamic tool updates"). `elicitation/create` is supported with two
modes (form, URL). Sampling not documented as supported — Anthropic has not
shipped server-callable sampling in Claude Code.

### Tool exposure / namespacing

Format `mcp__<server>__<tool>` (same shape as Codex). Tool descriptions and
server `instructions` are truncated at **2 KB each** ("Claude Code truncates
tool descriptions and server instructions at 2KB each. Keep them concise to
avoid truncation, and put critical details near the start.").

### Deferred loading — "MCP Tool Search"

Default-on for Sonnet 4+ / Opus 4+ (uses `tool_reference` content blocks).
Behaviour: at session start only **tool names** are sent to the model; the
schema/description for a specific tool is fetched on demand when the model
emits a `tool_reference` block requesting it. Disabled automatically on
Vertex AI and on non-first-party `ANTHROPIC_BASE_URL`. Env override:

| `ENABLE_TOOL_SEARCH` | Effect |
|---|---|
| (unset) | Defer-on-demand on direct API; load-upfront fallback elsewhere. |
| `true` | Force defer everywhere. |
| `false` | Disable. |

This is the closest thing in any of the prior-art to a real "lazy schema"
mechanism. It works because the API server itself participates — it's not
purely a client-side trick.

### Permission UX

Each MCP tool call asks the user before first use unless the user has
already chosen "always allow" for that tool. Granularity: **per-tool**, not
per-server. State persisted per workspace. There is no per-call prompt for
approved tools.

### Allowlist / denylist

Two layers:

1. **Server allow/deny** in `managed-mcp.json` (or a non-managed-mode
   admin policy file): `allowedMcpServers` / `deniedMcpServers`. Each entry
   matches by `serverName`, exact `serverCommand` array (must match
   element-wise — `["npx","-y","X"]` does **not** match `["npx","X"]`), or
   `serverUrl` glob (`https://*.example.com/*`). When the allowlist contains
   any `serverCommand` entry, stdio servers must match a command — not just
   a name. Empty `allowedMcpServers: []` is a complete lockdown. Denylist
   wins absolutely over allowlist.
2. **Tool-level**: `mcpServers.<name>.enabled[]` / `disabled[]` — same
   semantics as Codex's `enabled_tools` / `disabled_tools`.

### Reconnect / death

HTTP/SSE: exponential backoff, **5 attempts**, starts at 1 s, doubles each
time. Server shows as `pending` in `/mcp` during retry, `failed` after.
Stdio: docs only say "Stdio servers are local processes" (auto-restart not
guaranteed). User can retry from `/mcp`.

### Resources & Prompts

Yes both. Resources via `@server:resource://path` mentions in prompt input;
fuzzy-searchable autocomplete. Prompts surface as slash commands
`/<server>:<prompt>` with arg parsing from the prompt schema; result
injected into conversation. `notifications/resources/list_changed` and
`notifications/prompts/list_changed` are honoured.

### Sampling reverse-call

Not documented as supported. Safer default.

### Elicitation

Yes — form mode and URL mode. There's an `Elicitation` hook that lets the
user auto-respond instead of seeing the dialog (docs: "To auto-respond to
elicitation requests without showing a dialog, use the `Elicitation` hook").

### OAuth

Full OAuth 2.0 with auto-discovery (RFC 9728 → RFC 8414).
`--client-id` / `--client-secret` / `--callback-port` flags on
`claude mcp add`. Tokens in macOS Keychain or a credentials file on Linux.
`/mcp` runs the browser flow. `MCP_CLIENT_SECRET` env var skips the
interactive secret prompt. `--no-browser` style headless paths exist via
copy-paste callback URL.

### Logging / output

Stderr from stdio servers is surfaced in `/mcp` and the log files. Tool
output is capped by `MAX_MCP_OUTPUT_TOKENS` env var; per-tool override via
the `anthropic/maxResultSizeChars` annotation in the server's
`tools/list` response.

### "alwaysLoad"

The user's prompt referenced an `alwaysLoad` flag (2.1.121). The current
docs page does **not** name a field by that exact key — the closest concept
is the **deferred-by-default Tool Search** described above plus the
per-server option to opt out of it (so a server marked to always load
bypasses the deferred search). Treating `alwaysLoad: true` as an opt-out of
Tool Search per-server is the documented behaviour even if the field name
isn't reproduced verbatim in the public docs page.

---

## jcode

`1jehuang/jcode`. Rust workspace, `crates/jcode-*` plus a top-level `src/`
with the bulk of the harness. **Does not** depend on the `rmcp` SDK — has
its own hand-rolled MCP client.

### Crate map

MCP code lives **outside** the per-feature crates, under `src/mcp/`:

```
src/mcp/client.rs     // McpClient + McpHandle: per-server runtime
src/mcp/manager.rs    // McpManager: per-session aggregator
src/mcp/pool.rs       // SharedMcpPool: cross-session sharing for daemon mode
src/mcp/protocol.rs   // McpConfig, McpServerConfig, JSON-RPC types
src/mcp/tool.rs       // create_mcp_tools — wraps each MCP tool as jcode Tool
src/mcp/mod.rs        // re-exports
src/tool/mcp.rs       // McpManagementTool: agent-callable connect/disconnect/reload
```

### Config

`McpConfig` (`src/mcp/protocol.rs`):
```rust
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)] pub args: Vec<String>,
    #[serde(default)] pub env: HashMap<String, String>,
    #[serde(default = "default_shared")] pub shared: bool, // default true
}
```

JSON only — no HTTP transport. Stdio servers only.

Load order (`McpConfig::load`):
1. **First-run import** from `~/.claude/mcp.json` and
   `~/.codex/config.toml` (`[mcp_servers.*]` table) into
   `~/.jcode/mcp.json`. Codex entries override Claude entries on name
   collision. Runs once if `~/.jcode/mcp.json` doesn't exist.
2. Merge `~/.jcode/mcp.json` (global) → `.jcode/mcp.json` (project) →
   `.claude/mcp.json` (project-local Claude compat) — last write wins.

`shared: bool` distinguishes stateless (Todoist, Canvas) from stateful
(Playwright with browser state) servers. This is jcode-specific and
matters because:

### Spawn / pool

`McpManager` (`src/mcp/manager.rs`) is per-session. In **daemon mode** it
holds a reference to `SharedMcpPool` (`src/mcp/pool.rs`) — a process-global
`tokio::sync::OnceCell<Arc<SharedMcpPool>>`. For each configured server:

- `shared = true` → `SharedMcpPool::connect_server(name, config)` (deduplicated
  via `begin_connect` leader/wait pattern; one process spawn shared across
  every session). Manager keeps a weak `McpHandle`.
- `shared = false` → `McpClient::connect(name, config)` per-session. Owned
  client, dies with the session.

Standalone mode (TUI) uses no pool; every server is per-session.

### Handshake / wire protocol

`src/mcp/protocol.rs` defines its own JSON-RPC types — does not use rmcp.
Stdio framing is line-delimited JSON over child stdin/stdout. Spec coverage
is limited: tools yes, resources/prompts/sampling/elicitation **no**.

### Tool exposure / namespacing

`src/mcp/tool.rs::create_mcp_tools(manager) -> Vec<(String, Arc<dyn Tool>)>`.
Each MCP tool becomes a jcode `Tool` trait impl. Tool name format:
`mcp__<server>__<tool>` (matches Codex+Claude). Registry methods:
`Registry::register(name, tool)`, `Registry::unregister_prefix("mcp__<server>__")`
on disconnect. Tool list refreshed on `connect`/`reload`.

### Agent self-configuration

`src/tool/mcp.rs::McpManagementTool` is a single tool the agent itself can
call: `{action: "list" | "connect" | "disconnect" | "reload", server, command, args, env}`.
Lets the model spawn a new MCP server mid-session. `reload` re-reads
`~/.jcode/mcp.json` and reconciles. This is a notable divergence from
Codex+Claude — neither lets the model add a server.

### Permission UX

Not surfaced in the code paths read. The `McpManagementTool` has no
permission gating in the snippets searched; presumably the harness's
generic per-tool permission gate covers MCP-spawned tools the same as any
other.

### Allowlist / denylist, OAuth, sampling, elicitation, resources, prompts

None implemented. jcode is the leanest of the three.

### Reconnect / death

`disconnect_all` drains owned clients and releases pool handles. No
auto-restart visible in the snippets. `last_errors` map on the pool with a
`FAILED_CONNECT_RETRY_COOLDOWN` to suppress retry storms.

---

## pi-mono

Searched the repo (`gh search code 'mcp OR ModelContextProtocol' --repo badlogic/pi-mono`).
**No MCP code.** `packages/` has only `agent`, `ai`, `coding-agent`, `tui`,
`web-ui`. Tools are TS classes registered programmatically; extensions are
TS modules loaded via `jiti`. There is no client of any MCP server and no
MCP server impl.

This matches the pi-mono docs: extensions run with full Node permissions
(no sandbox), so MCP's value-prop (sandboxed third-party tool servers) is
weaker for them. They get extension hot-reload another way.

---

## Synthesis

### Common across all (≥2 of Codex/Claude/jcode)

1. **Tool naming convention `mcp__<server>__<tool>`.** Codex,
   Claude Code, and jcode all use this exact prefix. Hivecore should follow.
2. **Stdio first, HTTP second.** Every impl supports stdio child-process;
   only Codex+Claude do streamable HTTP. SSE is Claude-only.
3. **Per-server config block keyed by server name.** Differs in TOML vs
   JSON, but the shape is identical: `command`, `args`, `env` (and
   transport extras for HTTP).
4. **Per-tool allow/deny** (`enabled` + `disabled` lists, allow-by-default
   when allowlist absent). Codex + Claude verbatim.
5. **`tools/list_changed` is honoured.** Codex + Claude refresh on
   notification; jcode reloads on operator command. Never re-list every turn.
6. **Tool descriptions are truncated.** Claude at 2 KB; Codex enforces
   64-byte tool name + sanitization. Schema is rewritten before sending to
   the model.

### Divergences

1. **SDK vs hand-roll.** Codex uses `rmcp` (the official SDK). jcode
   hand-rolls. pi-mono skips. For a Rust project starting today the rmcp
   SDK choice is the cheaper one — Codex's wrapper is ~3 k LOC, jcode's
   is comparable, and the SDK keeps spec drift off your plate.
2. **Server placement.** Codex routes spawns through an exec-server
   abstraction so the same `RmcpClient` works against an in-process child
   or a sandboxed-process child. jcode spawns directly. Hivecore needs the
   abstraction (Layer 3 sandbox plane).
3. **Pool / session sharing.** Only jcode pools stateless servers across
   sessions. For a multi-tenant hivecore daemon this is mandatory — but
   hivecore's tenancy boundary is stricter than jcode's "session" boundary,
   so the pool must be **per-tenant**, not process-global.
4. **Sampling.** **Nobody implements it.** Even rmcp's default refuses with
   `method_not_found`. Treat sampling as v0.2+ at the earliest, and gate
   it behind a per-server config flag with a default of `false`.
5. **Tool Search / deferred loading.** Only Claude Code does it, and only
   because the API server cooperates via `tool_reference` blocks. Generic
   clients can't replicate this transparently. v0.2 substrate hook,
   not v0.1.
6. **Agent self-configuration.** jcode lets the model add servers via the
   `mcp` meta-tool. Codex+Claude don't. This feels footgun-shaped for a
   multi-tenant substrate (model spawns process with tenant ambient
   permissions). Skip in v0.1.
7. **Permission granularity.** Codex: per-tool TOML config. Claude:
   per-tool runtime "always-allow". jcode: none observed.

### What hivecore should/shouldn't replicate

**Replicate.** rmcp SDK use; `mcp__server__tool` naming;
`enabled_tools`/`disabled_tools` allow/deny; `serverCommand` array-match
allowlist policy (Claude pattern — gives admins a real lever); per-server
TOML in `~/.hivecore/config.toml` (Codex shape) plus per-project
`.hivecore/mcp.toml`; OAuth via rmcp `auth` feature with keyring storage;
honour `tools/list_changed`; truncate tool descriptions to a documented
budget; keychain-stored OAuth tokens.

**Don't replicate (yet).** Sampling (`create_message`); jcode's
"agent self-configures MCP" surface; MCP-via-HTTP-SSE-and-WebSocket all at
once; bespoke "Tool Search" without API-server cooperation.

---

## Recommendation for hivecore (concrete)

### Crate layout

Layer 2 only — MCP is harness substrate, not runtime.

```
crates/
  hivecore-mcp-client/        # Layer 2. Wraps rmcp. Per-server RmcpClient.
                              # Owns transports, OAuth, ClientHandler.
  hivecore-mcp-host/          # Layer 2. Per-tenant aggregator over N clients.
                              # Tool name qualification, allow/deny, cache,
                              # reconnect, lifecycle events to EventSink.
  hivecore-mcp-server/        # Optional Layer 3 binary — hivecore-as-MCP-server,
                              # exposes a hivecore agent over MCP/ACP.
                              # Defer to v0.2.
```

`hivecore-mcp-host` registers each MCP tool as a `runtime_core::Tool`
implementer at session start (jcode's `create_mcp_tools` pattern, but
typed against `hivecore-runtime-core`'s `Tool` trait — no separate
"MCP tool kind" inside the runtime).

### Trait surface (Layer 2)

```rust
// hivecore-mcp-client
pub struct McpClient { /* Mutex<state>, Semaphore for recovery */ }
impl McpClient {
    pub async fn new_stdio(spec: StdioServerSpec, launcher: Arc<dyn StdioLauncher>) -> Result<Self>;
    pub async fn new_http(url: Url, auth: Option<OAuthConfig>) -> Result<Self>;
    pub async fn initialize(&self, client_info: ClientInfo) -> Result<InitializeResult>;
    pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>>;
    pub async fn call_tool(&self, name: &str, args: JsonValue) -> Result<CallToolResult>;
    pub async fn list_resources(&self) -> Result<Vec<Resource>>;
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult>;
    pub async fn list_prompts(&self) -> Result<Vec<Prompt>>;
    pub async fn shutdown(self) -> Result<()>;
}

pub trait StdioLauncher: Send + Sync {  // Codex pattern
    fn launch(&self, spec: StdioServerSpec) -> BoxFuture<'static, io::Result<StdioServerTransport>>;
}

// hivecore-mcp-host
pub struct McpHost { tenant_id: TenantId, clients: HashMap<String, Arc<McpClient>>, ... }
impl McpHost {
    pub async fn from_config(config: &McpConfig, tenant_id: TenantId, ...) -> Self;
    pub async fn refresh_tools(&self) -> Vec<QualifiedTool>;
    pub async fn dispatch(&self, qualified_name: &str, args: JsonValue) -> Result<CallToolResult>;
    pub fn into_tools(&self) -> Vec<Box<dyn runtime_core::Tool>>;
}
```

`ClientHandler` impl in `hivecore-mcp-client` defaults: `create_message`
unimplemented, `list_roots` returns a tenant-scoped roots list (this is
the hook the Tenancy plane uses), `create_elicitation` routes to the
session's elicitation channel (UOK Gate plane), notifications fan into
the `EventSink`.

### Default config (TOML, lives at `~/.hivecore/config.toml` + `.hivecore/mcp.toml`)

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "${env:GITHUB_TOKEN}" }
enabled = true
startup_timeout_sec = 30
tool_timeout_sec = 60
default_tools_approval_mode = "on_request"
enabled_tools = []     # empty = no allowlist (allow all)
disabled_tools = []

[mcp_servers.github.tools.create_issue]
approval_mode = "always"
```

JSON in `.hivecore/mcp.json` accepted as Claude Code compatibility (jcode's
import-on-first-run pattern is worth copying; hivecore can import from
`~/.codex/config.toml`, `~/.claude.json`, `.mcp.json`).

### Permission UX (v0.1)

Default `on_request` — every MCP tool call goes through the Gate plane's
`PreToolCall` checkpoint, returning `Pass | RequiresApproval | Deny`.
Approval is a sub-agent prompt routed through the ACP server (so it
surfaces in any ACP client's UI). "Always-allow per tool" persisted in
`~/.hivecore/state.toml`. No per-call prompt for already-approved tools.

### Allowlist semantics (v0.1)

Two layers, in this order, deny wins:

1. **Server allowlist/denylist** (org policy, in `~/.hivecore/policy.toml`,
   not the per-server config). Match on `server_name`, exact
   `server_command` array, or `server_url` glob — copy Claude Code's
   semantics verbatim. Empty allowlist = full lockdown. Denylist absolute.
2. **Tool allowlist/denylist** (`enabled_tools` / `disabled_tools` per
   server) — Codex semantics.

### Tool-namespacing rule

Model-visible: `mcp__<sanitized_server>__<sanitized_tool>`, ≤ 64 bytes,
SHA-1 suffix on collision, raw `(server, tool)` kept in `ToolInfo` for
dispatch. Identical to Codex `qualify_tools`.

### Description budget

Hard-cap at 2 KiB per tool description and per server `instructions`,
matching Claude Code. Truncate with a trailing ellipsis + log a warning
including the server name. Cache the truncated form alongside the raw.

### `rmcp` Cargo features to enable (v0.1)

```toml
rmcp = { version = "x.y", default-features = false, features = [
    "client",
    "transport-child-process",
    "transport-io",
    "macros",
    "schemars",
] }
```

Add for v0.2: `auth`, `transport-streamable-http-client-reqwest`,
`elicitation`. Add only if needed: `auth-client-credentials-jwt`, `transport-ws`.

### Workspace dep entry

```toml
# crates/hivecore-mcp-client/Cargo.toml
[dependencies]
rmcp = { workspace = true, default-features = false, features = [
    "client", "transport-child-process", "transport-io", "macros", "schemars",
] }
hivecore-runtime-core = { workspace = true }
hivecore-config = { workspace = true }
tokio = { workspace = true, features = ["process", "sync", "macros", "rt"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }   # binary error path; mcp-client itself uses thiserror
```

Workspace root pins `rmcp` once.

### v0.1 scope vs v0.2 backlog

**v0.1 (in scope).**
- Stdio transport. Per-server TOML config under `[mcp_servers.*]`.
- `hivecore-mcp-client` over rmcp; `hivecore-mcp-host` aggregator.
- Tool name qualification + per-tool allow/deny + 2 KiB description cap.
- `tools/list_changed` cache invalidation.
- `ClientHandler::create_elicitation` wired to ACP elicitation channel.
- `ClientHandler::list_roots` wired to Tenancy plane (returns tenant-scoped
  workspace roots).
- Approval gate per tool through Gate plane (`Pass | RequiresApproval | Deny`).
- Stdio server stderr forwarded to the per-tenant audit log.
- First-run import from `~/.codex/config.toml` and `~/.claude.json`
  (jcode pattern; cheap; reduces onboarding friction).

**v0.2+ (backlog).**
- Streamable-HTTP + OAuth 2.0 (RFC 9728/8414) + keyring token storage.
- Per-tenant connection pool for stateless servers (jcode pattern, scoped
  to `tenant_id` not process).
- Server-name allowlist/denylist policy file.
- Resources + Prompts surfaces (the model hooks into `@<server>:<uri>`
  references and `/<server>:<prompt>` slash commands — Layer 3 substrate
  work).
- `notifications/resources/list_changed` + `prompts/list_changed`.
- Auto-reconnect with exponential backoff for HTTP transports (Claude Code
  parameters: 5 attempts, 1 s base, 2× factor).
- Hivecore-as-MCP-server (`hivecore-mcp-server` binary).
- Sampling (`create_message`) — only after a written security review:
  prompt-injection cost, model-credit cost, audit story.
- Deferred-load equivalent of Tool Search — only if/when hivecore's own
  model adapter supports tool-reference round-trips.

### Open questions

- Where does the approval gate live exactly — `hivecore-mcp-host` or the
  Gate plane in Layer 3? Recommendation: emit a typed `PreToolCall` event
  from the host, let Gate plane decide. Keep the host policy-free.
- Are MCP resources first-class in the KG (Code/Tribal subgraph entries)
  or pure model-time mentions? Decision needed before v0.2 resources work.
- Does hivecore's tenancy plane authorize per-tool calls, per-server
  connections, or both? Default position: both.
