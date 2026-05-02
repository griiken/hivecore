# MCP deep-dive — pi extensions / Codex lazy / GSD-2

Source: pi-mono@HEAD, openai/codex@HEAD, gsd-build/gsd-2@v2.78.1 (cloned 2026-05-01).
Companion to `.research/mcp-integration.md`.

---

## Part 1 — pi-mono extension architecture

### Confirm: MCP support — ABSENT (intentional, by author)

`packages/coding-agent/README.md:466`:
> **No MCP.** Build CLI tools with READMEs (see [Skills](#skills)), or build an extension that adds MCP support. [Why?](https://mariozechner.at/posts/2025-11-02-what-if-you-dont-need-mcp/)

`packages/coding-agent/docs/usage.md:275`:
> It intentionally does not include built-in MCP, sub-agents, permission popups, plan mode, to-dos, or background bash. You can build or install those workflows as extensions or packages, or use external tools such as containers and tmux.

The author has a published anti-MCP stance. The package `pi-mcp-adapter` (referenced in tests) is a separate optional bridge an end user can install — not part of the kernel.

### Tool registration mechanism

Tools register **only via extensions**. No "register tool by URL" or "spawn tool server" path exists.

Layered shape:
- `@mariozechner/pi-agent-core` defines `AgentTool` (the I/O-free trait surface in pi-mono).
- `@mariozechner/pi-coding-agent` ships built-in tools (`bash`, `read`, `write`, `edit`, `find`, `grep`, `ls`).
- Extensions return a `ToolDefinition[]` from a factory and are wrapped via `wrapToolDefinition()` (`packages/coding-agent/src/core/tools/tool-definition-wrapper.ts`).

`tool-definition-wrapper.ts` (full body):
```ts
export function wrapToolDefinition<TDetails = unknown>(
  definition: ToolDefinition<any, TDetails>,
  ctxFactory?: () => ExtensionContext,
): AgentTool<any, TDetails> {
  return {
    name: definition.name,
    label: definition.label,
    description: definition.description,
    parameters: definition.parameters,
    prepareArguments: definition.prepareArguments,
    executionMode: definition.executionMode,
    execute: (toolCallId, params, signal, onUpdate) =>
      definition.execute(toolCallId, params, signal, onUpdate, ctxFactory?.() as ExtensionContext),
  };
}
```

Trivial pass-through; no permission/sandbox shim at this seam.

### Subprocess-spawned tools — only `bash` / `find` / `grep`

```
packages/coding-agent/src/core/tools/bash.ts:7   import { spawn } from "child_process";
packages/coding-agent/src/core/tools/find.ts:4   import { spawn } from "child_process";
packages/coding-agent/src/core/tools/grep.ts:4   import { spawn } from "child_process";
packages/coding-agent/src/core/exec.ts:5         import { spawn } from "node:child_process";
packages/coding-agent/src/core/package-manager.ts:1   spawn, spawnSync …
```

`bash.ts` spawns the user's shell with `-c <command>`. `find.ts` shells `fd`. `grep.ts` shells `rg`. There is **no generic "tool server" subprocess pattern** — every spawn is hard-coded inside a specific built-in tool implementation.

There is a `BashSpawnHook` (`bash.ts:147`) that lets an extension intercept the bash command before exec. It is not a sandbox — it is a transformation hook (e.g. wrap with `firejail`, or a recording shim).

### Extension loading model — eager, in-process, no sandbox

`packages/coding-agent/src/core/extensions/loader.ts` uses `@mariozechner/jiti` (a fork of jiti) to load TS extension files at process start. Files:

```
packages/coding-agent/src/core/extensions/
   loader.ts    606 LoC  — discoverAndLoadExtensions / loadExtensions
   runner.ts   1068 LoC  — ExtensionRunner: lifecycle dispatch
   types.ts    1563 LoC  — Extension, ExtensionAPI, ToolDefinition, events
   wrapper.ts    30 LoC  — wrap ToolDefinition→AgentTool
   index.ts     172 LoC  — public re-exports
```

Loader uses `virtualModules` so the compiled Bun binary still resolves `@mariozechner/pi-agent-core`, `pi-ai`, `pi-tui`, `pi-coding-agent`, `typebox` from extension imports. Discovery walks the user config dir (`~/.pi/extensions/`) for TS files.

**Trust model: full Node permissions.** The author documents this in usage.md and accepts it because pi is single-user.

### Description / schema handling

- Schemas are TypeBox `TSchema`. Pi forwards them straight to the LLM via `pi-ai` provider adapters; no truncation, no name sanitization, no $ref resolution.
- No description size cap in pi.
- No prompt-list / cache-bust mechanism — the agent loop re-reads the registry each turn (registry is just an in-memory `AgentTool[]` array on `AgentState.tools`).

### Permission gates

`grep -rn 'allowedTools|disallowedTools|toolPermission|approval' packages/coding-agent/src` returned **zero hits** (excluding tests). Permission gating is delegated entirely to:

1. The `BeforeToolCallContext` hook surface (extension can return `{ skip: true }` or transform args).
2. `executionMode: 'sequential' | 'parallel'` per tool.
3. The user (interactive prompt; coding-agent does not auto-approve).

### Five-bullet summary — what pi does instead of MCP

- **Skills, not tool servers.** A "skill" is a markdown README + bash CLI. The model invokes the CLI through `bash` and reads the README. No protocol.
- **In-process TypeScript extensions** (jiti) for adding new tools, hooks, commands, autocompletes — fast iteration, full host trust.
- **Three hard-coded subprocess tools** (`bash`, `find→fd`, `grep→rg`); everything else lives in the host process.
- **One spawn hook** (`BashSpawnHook`) lets an extension wrap the shell command without re-implementing exec.
- **Hostile-to-MCP positioning** — the author publicly argues MCP is unnecessary for this shape of agent and refuses to ship built-in MCP support. Closest analog: `pi-mcp-adapter` as an optional extension.

---

## Part 2 — Codex lazy loading + sanitization (with code)

All paths under `codex-rs/codex-mcp/src/` unless noted.

### `qualify_tools` full body

`tools.rs` (truncated docs preserved):

```rust
//! MCP tool metadata, filtering, schema shaping, and name qualification.
//!
//! Raw MCP tool identities must be preserved for protocol calls, while
//! model-visible tool names must be sanitized, deduplicated, and kept within API
//! limits.

const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;
const CALLABLE_NAME_HASH_LEN: usize = 12;

pub(crate) fn qualify_tools<I>(tools: I) -> HashMap<String, ToolInfo>
where
    I: IntoIterator<Item = ToolInfo>,
{
    let mut seen_raw_names = HashSet::new();
    let mut candidates = Vec::new();
    for tool in tools {
        let raw_namespace_identity = format!(
            "{}\0{}\0{}",
            tool.server_name, tool.connector_id.as_deref().unwrap_or(""), tool.callable_namespace
        );
        // ... build CallableToolCandidate per tool, dedup by raw identity ...
    }

    // Detect collisions where two distinct raw identities sanitize to the same
    // (callable_namespace, callable_name); mark them for hashing.
    let mut tool_identities_by_base = HashMap::<(String, String), HashSet<String>>::new();
    for candidate in &candidates {
        tool_identities_by_base
            .entry((candidate.callable_namespace.clone(),
                    candidate.callable_name.clone()))
            .or_default()
            .insert(candidate.raw_tool_identity.clone());
    }
    let colliding_tools = tool_identities_by_base
        .into_iter()
        .filter_map(|(key, identities)| (identities.len() > 1).then_some(key))
        .collect::<HashSet<_>>();

    candidates.sort_by(|l, r| l.raw_tool_identity.cmp(&r.raw_tool_identity));

    let mut used_names = HashSet::new();
    let mut qualified_tools = HashMap::new();
    for mut candidate in candidates {
        let (callable_namespace, callable_name, qualified_name) = unique_callable_parts(
            &candidate.callable_namespace,
            &candidate.callable_name,
            &candidate.raw_tool_identity,
            &mut used_names,
        );
        candidate.tool.callable_namespace = callable_namespace;
        candidate.tool.callable_name = callable_name;
        qualified_tools.insert(qualified_name, candidate.tool);
    }
    qualified_tools
}
```

Companion (line 200-249):
```rust
fn truncate_name(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn fit_callable_parts_with_hash(
    namespace: &str, tool_name: &str, raw_identity: &str,
) -> (String, String) {
    let suffix = callable_name_hash_suffix(raw_identity);
    let max_tool_len = MAX_TOOL_NAME_LENGTH.saturating_sub(namespace.len());
    if max_tool_len >= suffix.len() {
        let prefix_len = max_tool_len - suffix.len();
        return (namespace.to_string(),
                format!("{}{}", truncate_name(tool_name, prefix_len), suffix));
    }
    let max_namespace_len = MAX_TOOL_NAME_LENGTH - suffix.len();
    (truncate_name(namespace, max_namespace_len), suffix)
}

fn unique_callable_parts(
    namespace: &str, tool_name: &str, raw_identity: &str,
    used_names: &mut HashSet<String>,
) -> (String, String, String) {
    let qualified_name = format!("{namespace}{tool_name}");
    if qualified_name.len() <= MAX_TOOL_NAME_LENGTH
        && used_names.insert(qualified_name.clone())
    {
        return (namespace.to_string(), tool_name.to_string(), qualified_name);
    }
    let mut attempt = 0_u32;
    loop {
        let hash_input = if attempt == 0 {
            raw_identity.to_string()
        } else { format!("{raw_identity}\0{attempt}") };
        let (namespace, tool_name) =
            fit_callable_parts_with_hash(namespace, tool_name, &hash_input);
        let qualified_name = format!("{namespace}{tool_name}");
        if used_names.insert(qualified_name.clone()) {
            return (namespace, tool_name, qualified_name);
        }
        attempt = attempt.saturating_add(1);
    }
}
```

### Sanitization regex / chars

`codex-rs/utils/plugins/src/mcp_connector.rs:31` (and dup at `connectors/src/metadata.rs:15`):

```rust
pub fn sanitize_name(name: &str) -> String {
    sanitize_slug(name).replace("-", "_")
}

fn sanitize_slug(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() { "app".to_string() } else { normalized.to_string() }
}
```

Algorithm: lowercase ASCII alphanum kept; **everything else (including unicode letters)** becomes `-`; trim leading/trailing `-`; empty → `"app"`; finally `-` → `_`. So allowed final chars are `[a-z0-9_]` only. Underscore-as-separator is a deliberate post-processing step so the wire-format namespace delimiter `__` is preserved cleanly between segments.

### 64-byte truncation + SHA-1 collision suffix

- Cap = `MAX_TOOL_NAME_LENGTH = 64` bytes (counted via `chars().take(n)`, so technically code-points; safe for the ASCII-only output of `sanitize_name`).
- Suffix length = `CALLABLE_NAME_HASH_LEN = 12` ASCII chars.
- Suffix value = first 12 chars of `Sha1(raw_identity)` hex.
- Trigger: only when `unique_callable_parts` finds the qualified name either exceeds 64 bytes **or** collides with an already-issued name in this batch.
- Truncation strategy: namespace is preserved if possible; only `tool_name` is truncated (`max_tool_len = 64 - namespace.len()`). If even namespace+suffix > 64, namespace itself is truncated and the tool-name portion becomes the suffix alone.
- On hash collision (extremely rare), `attempt` counter is appended to the SHA-1 input and re-hashed.

### Description rule

There is **no global "max description bytes" cap** for MCP tool descriptions. The only mutation is **annotation**, in `rmcp_client.rs:272-289`:

```rust
let description = tool.tool.description.as_deref().map(str::trim).unwrap_or("");
let annotated_description = if description.is_empty() {
    plugin_source_note
} else if matches!(description.chars().last(), Some('.' | '!' | '?')) {
    format!("{description} {plugin_source_note}")
} else {
    format!("{description}. {plugin_source_note}")
};
tool.tool.description = Some(Cow::Owned(annotated_description));
```

Plugin provenance ("This tool is part of plugin `X`.") is appended; otherwise descriptions flow through unchanged. Compare:
- Claude Code: 1 KiB MCP-tool description cap (per third-party reverse-engineering).
- Codex: no cap.
- Codex's only hard byte budget on the model side is `TELEMETRY_PREVIEW_MAX_BYTES = 2 * 1024` (`core/src/tools/mod.rs:25`) — that's **telemetry preview**, not the prompt-side description.
- `core/src/tools/handlers/mcp.rs:185` sets `TruncationPolicy::Bytes(1024)` — that is the **tool-result** truncation cap, not description.

### Schema rewrite

Two transforms before the schema reaches the model:

1. `tool_with_model_visible_input_schema` (`tools.rs:80-105`): if MCP `_meta` declares `openai/fileParams: ["path", ...]`, replaces those fields' subschema with a `string` (or `array<string>`) and appends fixed guidance: *"This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."* This is a **schema rewrite for OpenAI Files API integration**, not generic JSON-Schema normalization.

2. `mask_input_property_schema`: clears the entire property object and rewrites it to `{ type: "string" | "array of string", description: <guidance> }`. So **types are coerced** for declared file params.

What is **not** done:
- No `$ref` resolution.
- No `default` inlining.
- No type widening / pruning of unknown JSON-Schema keywords.
- No description-size truncation.

The schema is otherwise the raw `Tool.input_schema` (`Arc<serde_json::Map>`) handed to the model.

### Lazy loading — partial yes

- **Listing**: `tools/list` is called eagerly at MCP server connection time; results cached in `managed_client.listed_tools()` (in-memory).
- **Codex Apps tools cache**: persistent on-disk cache (`codex_apps_tools_cache_context`), see `codex_apps.rs:158/177/187/211` (`write_cached_codex_apps_tools` / `load_cached_codex_apps_tools`). Loaded via a `CACHE_HIT` fast path. So *cold-start* tool listing for Codex Apps is genuinely lazy w.r.t. the network.
- **Schema attachment**: there is a `tool_namespace`-level "deferred tool loading" comment (`tools.rs` ToolInfo doc: *"Model-visible namespace used for deferred tool loading"*) — meaning Codex can advertise just a namespace prefix to the model and lazy-fetch full schemas per-namespace, but this is mostly used for the Codex Apps surface not generic stdio MCP.

Effectively: **eager list at connect; cached-on-disk for Codex Apps; no per-tool lazy fetch on plain stdio servers.**

### `tools/list_changed` handling

Server **emits** `tools/list_changed` (`mcp-server/src/message_processor.rs:246` advertises capability). Client **does not act on it** — `rmcp-client/src/logging_client_handler.rs:83-85`:

```rust
async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
    info!("MCP server tool list changed");
}
```

Just a log line. No cache invalidation, no re-list. To force refresh, callers invoke `connection_manager.rs:337 hard_refresh_codex_apps_tools_cache()` explicitly. So Codex's behaviour is: **trust the cache until something explicitly says refresh** — which means a server that hot-swaps tools is silently out-of-sync until next session.

### Allow/deny enforcement

`tools.rs:80-100`:

```rust
pub(crate) struct ToolFilter {
    pub(crate) enabled: Option<HashSet<String>>,
    pub(crate) disabled: HashSet<String>,
}

impl ToolFilter {
    pub(crate) fn allows(&self, tool_name: &str) -> bool {
        if let Some(enabled) = &self.enabled {
            if !enabled.contains(tool_name) { return false; }
        }
        !self.disabled.contains(tool_name)
    }
}

pub(crate) fn filter_tools(tools: Vec<ToolInfo>, filter: &ToolFilter) -> Vec<ToolInfo> {
    tools.into_iter().filter(|t| filter.allows(&t.tool.name)).collect()
}
```

Enforced in `connection_manager.rs:329` and after-cache-load in `rmcp_client.rs` — i.e. **before** `qualify_tools`, **before** the model ever sees the tool. `enabled_tools` / `disabled_tools` come from `McpServerConfig` (TOML).

Note: filtering uses **raw** `tool.name`, not the qualified `mcp__server__tool` name. Author of policy speaks server-native names.

### Per-tool approval mode

Approval is **shell-tool-centric**, not generic-MCP-tool-centric. `core/src/tools/runtimes/shell/unix_escalation.rs` repeatedly references `approval_policy: AskForApproval` (variant of `OnRequest`/`Never`/etc.). For MCP tools there is no per-tool persistence — each call goes through the shared `ToolFilter` allow/deny only; there is no "remember 'always allow this tool' across runs" record.

Storage: `approval_policy` lives on `TurnContext` (in-process, per turn). It is **not persisted** between sessions for MCP tools. Shell-command approvals (auto-approve list) live in config TOML (e.g. `core/src/config/mod.rs`).

### Tool dispatch path

`core/src/tools/handlers/mcp.rs` (223 LoC) `McpHandler` receives `ToolPayload::Mcp { server, tool, raw_arguments }`, looks up the connection by `server`, calls native MCP `tools/call`, applies `TruncationPolicy::Bytes(1024)` to the result, and returns `McpToolOutput`. Hook surface (`post_tool_use_payload`) maps `mcp__server__tool` qualified name back to the model-visible name for hook events.

---

## Part 3 — GSD-2

### Repo location + architecture

`github.com/gsd-build/gsd-2`, package name **`gsd-pi`** v2.78.1 (note: the package is *named* gsd-pi but the repo is gsd-2; same artifact). License MIT.

Structure:
```
src/                       — gsd-cli host
packages/
  pi-tui/                  — vendored fork of pi-tui
  pi-ai/                   — vendored fork of pi-ai
  pi-agent-core/           — vendored fork of pi-agent-core
  pi-coding-agent/         — vendored fork of pi-coding-agent
  mcp-server/              — @gsd-build/mcp-server (binary gsd-mcp-server)
  daemon/                  — gsd cloud daemon client
  rpc-client/, native/
extensions/                — extension workspaces
studio/                    — Next.js webapp
gsd-orchestrator/          — orchestrator service
```

GSD-2 is a **fork** of pi-mono, not a downstream — it vendored the four `pi-*` packages into its own monorepo (per ADR-016 in hivecore docs, "GSD-2 is itself one large extension on top of `pi-coding-agent`" — true conceptually but mechanically it is a fork-and-extend). They keep the package names `@gsd/pi-tui` etc.

### MCP integration — YES (server side only)

GSD-2 ships **`packages/mcp-server`** (= `@gsd-build/mcp-server`, binary `gsd-mcp-server`) — an **outbound** MCP server that exposes GSD orchestration tools (read, write, edit, bash, grep, glob, ls, plus workflow tools) to external MCP clients (Claude Code, Cursor, etc.). Package description literally:

> "MCP server exposing GSD orchestration tools for Claude Code, Cursor, and other MCP clients"

Files in `packages/mcp-server/src`: `server.ts`, `cli.ts`, `workflow-tools.ts`, `session-manager.ts`, `tool-credentials.ts`, `remote-questions.ts`, `env-writer.ts`, `readers/`. No `client.ts` / `connection-manager.ts` / inbound MCP machinery.

`src/mcp-server.ts` (host-side) is GSD's **own glue** to start the bundled `@gsd-build/mcp-server` over stdio. Code path uses `@modelcontextprotocol/sdk` server APIs (`server/stdio` etc.).

### Inbound MCP client (consume external MCP servers) — NO

`grep -rln 'McpClient|StdioClientTransport' packages/pi-coding-agent src` returned **zero non-test hits**. GSD-2 inherits pi-mono's no-inbound-MCP stance — it does not consume third-party MCP servers as tools.

### Tool-server pattern — extensions, not subprocesses

GSD-2's extensions framework is `extensions/*` workspaces + topological load order (Kahn's algorithm), with first reference extension `@gsd-extensions/google-search` carved out of core. CHANGELOG v2.78 explicitly:
- "Topological extension load order — Kahn's-algorithm sort with surfaced ExtensionLoadWarning's"
- "cmux ↔ gsd decoupling — static cross-imports replaced with a shared cmux-events contract and dynamic imports"
- "Unified component system — skills, agents, pipelines, and marketplace are now one component model"

Architecture is the same in-process jiti-loaded TS extension shape as pi, with topo-ordering layered on. No subprocess sandbox, no MCP-as-internal-tool-protocol.

### What hivecore can / can't learn from GSD-2

**Can learn:**
- **Outbound MCP server is the right shape for distribution.** GSD-2 ships `gsd-mcp-server` as a separate npm binary that exposes the *agent's tools* to external clients. Hivecore should plan an analogous `hivecore-mcp` binary so a hivecore harness can be mounted into Claude Code / Cursor like GSD already is.
- **`structuredContent` discipline.** GSD's `isPlainObject` guard for what gets forwarded to the MCP transport (mirroring it across `mcp-server` and `workflow-tools` because the protocol drops non-standard fields) is a real bug-vector worth pre-empting.
- **Topo-sorted extension load** with surfaced warnings (Kahn) instead of pi-mono's load-order-dependent silence.
- **Component unification** — skills/agents/pipelines as one model wired through dispatch+telemetry. Aligns with hivecore ADR-023 (skills-as-tools).

**Can't learn (because absent):**
- **Inbound MCP client.** GSD-2 does not consume MCP servers as tools, so no patterns for connection management, schema sanitization, allow/deny, or list_changed handling are available there. Codex remains hivecore's only OSS reference for inbound MCP.
- **Sandboxing.** Trust model is full-host-permission, same as pi.
- **Multi-tenancy.** Single-user assumptions throughout.

---

## Synthesis

Five takeaways for hivecore.

1. **Inbound MCP reference is Codex; not pi, not GSD-2.** Neither pi-mono nor GSD-2 implements an inbound MCP client. Anything hivecore does for tenant-supplied MCP servers must be modelled on Codex's `codex-mcp` / `rmcp-client` crates. Adopt: `qualify_tools` 64-char cap + 12-char SHA-1 suffix, `ToolFilter` allow/deny enforced before `qualify_tools`, raw-name vs qualified-name separation in `ToolInfo`. **Reject:** Codex's silent `on_tool_list_changed` handler — hivecore's tenancy plane needs cache invalidation on that signal, not just a log line.

2. **Outbound MCP server is the right OSS distribution shape.** GSD-2 ships `gsd-mcp-server` as a binary so Claude Code / Cursor can consume it. Hivecore should plan a `hivecore-mcp-server` binary that exposes the agent's tool surface — this is *separate* from `hivecore-acp-server` (ADR-018) which speaks ACP for editor↔agent. Both can coexist (different protocols, different consumers).

3. **Description-cap policy is hivecore's call.** Codex sets no description cap; Claude Code reportedly caps at ~1 KiB. Hivecore should pick **2 KiB hard cap with truncation marker** at v0.1 and revisit when description-budget across many tenant MCP servers becomes the binding constraint. Document in an ADR.

4. **Schema rewrite is minimal in Codex; do less, not more.** Codex only rewrites `openai/fileParams`-tagged properties. Resist the urge to "normalize JSON Schema" in hivecore's MCP client — the model is the consumer, and adding $ref resolution / default inlining will fight the rmcp/serde_json shape unnecessarily. Adopt the *non-mutation* default, with a registered `SchemaShaper` extension point only for explicit cases (file-param coercion, multi-tenant secret redaction).

5. **Permission persistence is a hivecore-original problem.** Codex stores per-call approval in `TurnContext` only; no persisted "always allow this MCP tool" record. For multi-tenant retrieval + audit (UOK ADR-019), hivecore needs persisted per-(tenant, server, tool) approval state in the Audit/Tenancy planes — neither prior art ships this. Wire it as an `approval` event class in the JSONL audit log + an `approval_decisions` table in the SQLite projection. Pi-mono's *nothing* and Codex's *in-memory only* are both insufficient for hivecore's threat model.

---

File: `/home/dipendra-sharma/projects/hivecore/.research/mcp-deep-dive-2.md`
