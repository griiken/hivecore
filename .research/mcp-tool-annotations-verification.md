# MCP Tool Annotations — Verification

**Date:** 2026-05-03 · **Confidence:** HIGH · **Purpose:** Wire `RiskHint.read_only` from MCP `ToolAnnotations.readOnlyHint`.

## TL;DR

- Spec field names are **camelCase** on the wire: `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`. All `boolean`, all optional, all *hints*.
- rmcp 0.8 `Tool` has `annotations: Option<ToolAnnotations>`; rust struct uses **snake_case** with `#[serde(rename_all = "camelCase")]`.
- Spec normative MUST: clients MUST treat annotations as untrusted unless from a trusted server. Hivecore should use `read_only_hint=true` as **input to policy**, never as auto-allow.
- Reference test server (`src/everything`) does **not** ship annotations on any tool. Reference filesystem server does — read tools `readOnlyHint:true`, write/edit `readOnlyHint:false` + `destructiveHint:true`.
- Goose path the user cited (`src/permission/permission.rs:101-117`) is wrong — **the file doesn't exist**. The real `apply_tool_annotations` lives in two places (see §4).

---

## 1. MCP spec — field shape + trust language

**Source:** `modelcontextprotocol/modelcontextprotocol@main` — `docs/specification/2025-06-18/server/tools.mdx`
URL: https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/docs/specification/2025-06-18/server/tools.mdx

Spec fields on `tool` object: `name`, `title`, `description`, `inputSchema`, `outputSchema`, `annotations`.

### Verbatim normative quote (tools.mdx)

> For trust & safety and security, clients **MUST** consider
> tool annotations to be untrusted unless they come from trusted servers.

(Wrapped in a `<Warning>` block — directly under the annotations field listing.)

### Schema (machine-checkable)

**Source:** `modelcontextprotocol/modelcontextprotocol@main` — `schema/2025-06-18/schema.json`
URL: https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/2025-06-18/schema.json

```json
"ToolAnnotations": {
  "properties": {
    "readOnlyHint":     { "type": "boolean", "description": "If true, the tool does not modify its environment.\n\nDefault: false" },
    "destructiveHint":  { "type": "boolean", "description": "If true, ... destructive updates ... (only meaningful when readOnlyHint == false)\nDefault: true" },
    "idempotentHint":   { "type": "boolean", "description": "... repeatedly with same arguments will have no additional effect ... (only meaningful when readOnlyHint == false)\nDefault: false" },
    "openWorldHint":    { "type": "boolean", "description": "... interact with an \"open world\" of external entities ...\nDefault: true" },
    "title":            { "type": "string" }
  }
}
```

**Defaults matter for risk inference:** if `readOnlyHint` absent, treat as `false` (i.e. assume write). If `destructiveHint` absent (and not read-only), treat as `true`.

---

## 2. rmcp 0.8 Rust SDK — exposed types

**Source:** `modelcontextprotocol/rust-sdk@main` — `crates/rmcp/src/model/tool.rs`
URL: https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp/src/model/tool.rs

### `Tool` struct (lines 12–43)

```rust
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Tool {
    pub name: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")] pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub description: Option<Cow<'static, str>>,
    pub input_schema: Arc<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")] pub output_schema: Option<Arc<JsonObject>>,
    #[serde(skip_serializing_if = "Option::is_none")] pub annotations: Option<ToolAnnotations>,
    #[serde(skip_serializing_if = "Option::is_none")] pub execution: Option<ToolExecution>,
    #[serde(skip_serializing_if = "Option::is_none")] pub icons: Option<Vec<Icon>>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")] pub meta: Option<Meta>,
}
```

→ `tool.annotations` is `Option<ToolAnnotations>`. Always check `.is_some()` first.

### `ToolAnnotations` struct (lines 101–151)

```rust
/// Clients should never make tool use decisions based on ToolAnnotations
/// received from untrusted servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ToolAnnotations {
    pub title: Option<String>,
    pub read_only_hint: Option<bool>,    // Default: false
    pub destructive_hint: Option<bool>,  // Default: true (when !read_only)
    pub idempotent_hint: Option<bool>,   // Default: false
    pub open_world_hint: Option<bool>,   // Default: true
}
```

### Convenience methods (lines 209–217)

```rust
pub fn is_destructive(&self) -> bool { self.destructive_hint.unwrap_or(true) }
pub fn is_idempotent(&self) -> bool  { self.idempotent_hint.unwrap_or(false) }
```

→ rmcp does **not** ship an `is_read_only()` helper. Hivecore should write its own; do not unwrap_or with the wrong default.

---

## 3. Real servers shipping annotations

### `modelcontextprotocol/servers` — `src/everything` (reference test server)

**Source:** GitHub tree listing + per-tool grep across `src/everything/tools/*.ts`
URL: https://github.com/modelcontextprotocol/servers/tree/main/src/everything/tools

Files checked: `echo.ts`, `get-sum.ts`, `get-tiny-image.ts`, `gzip-file-as-resource.ts`, `trigger-long-running-operation.ts`. **None set annotations.** Every grep for `readOnlyHint|destructiveHint|idempotentHint|openWorldHint|annotations` returned zero hits in tool registration files.

→ **Implication for Hivecore tests:** spinning up `@modelcontextprotocol/server-everything` will yield tools with `tool.annotations == None`. Risk-hint mapping must handle absent annotations gracefully (treat as worst-case: not read-only, destructive).

### `modelcontextprotocol/servers` — `src/filesystem`

**Source:** `src/filesystem/index.ts`
URL: https://raw.githubusercontent.com/modelcontextprotocol/servers/main/src/filesystem/index.ts

Verbatim registration excerpts:

```ts
// Read tools (lines 220, 243, 265, 316)
annotations: { readOnlyHint: true }

// write_file (line 352)
annotations: { readOnlyHint: false, idempotentHint: true, destructiveHint: true }

// edit_file (line 382)
annotations: { readOnlyHint: false, idempotentHint: false, destructiveHint: true }
```

→ Canonical pattern: read tools set ONLY `readOnlyHint:true`. Write tools set the explicit triple `{readOnlyHint:false, idempotentHint:?, destructiveHint:true}`. Other hint fields stay absent → consumer falls back to spec defaults.

### `modelcontextprotocol/servers-archived` — `src/git`

Archived repo path `servers-archived/main/src/git/src/mcp_server_git/server.py` returned 404 on raw fetch (path may have moved). Skipped — not load-bearing for Hivecore wiring.

---

## 4. Goose consumption (the real file paths)

User cited `crates/goose/src/permission/permission.rs:101-117`. **That file does not exist.** Goose has `permission/mod.rs` (285 bytes, no annotation logic) and `permission/permission_inspector.rs`. The actual `apply_tool_annotations` definitions live elsewhere.

### File 1 — `crates/goose/src/config/permission.rs:101-117` (PermissionManager — global cache)

URL: https://raw.githubusercontent.com/block/goose/main/crates/goose/src/config/permission.rs

```rust
pub fn apply_tool_annotations(&self, tools: &[Tool]) {
    let mut write_annotated = Vec::new();
    for tool in tools {
        let Some(anns) = &tool.annotations else { continue; };
        if anns.read_only_hint == Some(false) {
            write_annotated.push(tool.name.to_string());
        }
    }
    if !write_annotated.is_empty() {
        self.bulk_update_smart_approve_permissions(
            &write_annotated,
            PermissionLevel::AskBefore,
        );
    }
}
```

### File 2 — `crates/goose/src/permission/permission_inspector.rs:32-44` (PermissionInspector — per-agent cache)

URL: https://raw.githubusercontent.com/block/goose/main/crates/goose/src/permission/permission_inspector.rs

```rust
// readonly_tools is per-agent to avoid concurrent session clobbering; write-annotated
// tools are cached globally via PermissionManager.
pub fn apply_tool_annotations(&self, tools: &[Tool]) {
    let mut readonly_annotated = HashSet::new();
    for tool in tools {
        let Some(anns) = &tool.annotations else { continue; };
        if anns.read_only_hint == Some(true) {
            readonly_annotated.insert(tool.name.to_string());
        }
    }
    *self.readonly_tools.write().unwrap() = readonly_annotated;
    self.permission_manager.apply_tool_annotations(tools);
}
```

### How Goose interprets the bits

| Tool annotation | Goose action |
|---|---|
| `read_only_hint == Some(true)` | Add tool name to per-agent `readonly_tools` set. Used by `is_readonly_annotated_tool()` to **skip approval prompts** in smart-approve mode. |
| `read_only_hint == Some(false)` | Add to "smart_approve" config under `PermissionLevel::AskBefore`. |
| `None` (annotations absent) | **`continue`** — no policy change. Falls through to whatever the default permission level is. |

→ Goose answers the subtle question: `read_only_hint=true` is **policy input**, used to opt INTO auto-allow only when the user has separately enabled smart-approve mode. It is never a unilateral auto-allow. `read_only_hint=false` does NOT block — only escalates to AskBefore. `destructive_hint`, `idempotent_hint`, `open_world_hint` are **not consumed** by Goose's permission logic at all (verified: no reads of `destructive_hint` or `idempotent_hint` in either file).

---

## 5. Subtle recommendation for Hivecore `RiskHint.read_only`

Convention from spec + Goose + filesystem server:

1. **Source field:** `tool.annotations.read_only_hint` (snake_case in rmcp; serializes as `readOnlyHint`).
2. **Three-state mapping** (don't collapse to `bool`):
   - `Some(true)`  → `RiskHint { read_only: true,  source: AnnotationsTrusted | AnnotationsUntrusted }`
   - `Some(false)` → `RiskHint { read_only: false, ... }`
   - `None`        → `RiskHint { read_only: false, source: Default }` *(spec default is `false`)*
3. **Trust tagging is mandatory.** Spec MUST: untrusted unless server is trusted. Hivecore should carry a `server_trust: Trusted | Untrusted` flag on the `mcp.toml` server entry; `RiskHint` from an untrusted server feeds policy *as input*, never as auto-allow.
4. **Don't ignore the other three hints.** `destructive_hint`, `idempotent_hint`, `open_world_hint` are absent from Goose's logic but are present in the wire — Hivecore can use them for finer-grained risk scoring (e.g. a tool with `destructive_hint:true` + `idempotent_hint:false` is the riskiest class — first-time irreversible writes).
5. **Test fixture caveat:** `server-everything` ships zero annotations. For end-to-end risk tests, use `server-filesystem` or hand-craft a fixture server.

---

## 6. Sources (one-line bookmarks per ADR-010)

- MCP spec mdx (2025-06-18, tools): `modelcontextprotocol/modelcontextprotocol@main:docs/specification/2025-06-18/server/tools.mdx` — MUST-untrusted quote, field listing
- MCP schema JSON: `modelcontextprotocol/modelcontextprotocol@main:schema/2025-06-18/schema.json` — `ToolAnnotations` properties + defaults
- rmcp Tool/ToolAnnotations: `modelcontextprotocol/rust-sdk@main:crates/rmcp/src/model/tool.rs` — lines 12-43 (Tool), 101-151 (ToolAnnotations), 209-217 (helpers)
- Filesystem server registrations: `modelcontextprotocol/servers@main:src/filesystem/index.ts` — lines 220/243/265/316 (read), 352 (write), 382 (edit)
- Everything server (no annotations): `modelcontextprotocol/servers@main:src/everything/tools/*.ts` — verified absent
- Goose global permission cache: `block/goose@main:crates/goose/src/config/permission.rs:101-117`
- Goose per-agent inspector: `block/goose@main:crates/goose/src/permission/permission_inspector.rs:32-44`
