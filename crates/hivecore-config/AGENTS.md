# crates/hivecore-config/AGENTS.md

**Layer 3 — TOML agent registry.** Parses, validates, and serves `Agent` definitions (system prompt + model + tool policy + limits).

## Naming note

Industry convention is **agent**, not "persona". Claude Code stores these as `.claude/agents/<name>.md`; Codex / Zed / pi / GSD all use the same noun. Hivecore picks the same noun. The TOML key is `[agent]`. The earlier "Persona" naming has been retired (see CHANGELOG).

## What lives here

| Module | Purpose |
|---|---|
| `agent` | `Agent` (the validated record), `ModelSelection`, `ToolPolicy`, `ToolMode`, `Limits`. |
| `loader` | `AgentLoader` — TOML deserialisation + validation. Resolves `system_prompt.path` relative to the agent file. |
| `registry` | `AgentRegistry` — unique-id enforcement + insertion order preservation. |
| `error` | `ConfigError` + `ValidationError`. |

## TOML schema

```toml
[agent]
id          = "default"          # ^[a-z][a-z0-9_-]*$
name        = "Default SWE Assistant"
version     = "0.1.0"
description = "…"

[agent.model]
provider = "openai"              # v0.1: only "openai"
id       = "gpt-5.4-nano"

[agent.system_prompt]
inline = "…"                     # OR: path = "swe.md"  (relative to this file)

[agent.tools]
mode = "allowlist"               # allowlist | all | denylist
list = ["read_file", "edit_file", "bash"]

[agent.limits]
max_iterations = 16
```

## Builtins shipped

| File | Used by |
|---|---|
| `agents/builtin/default.toml` | Main session — full SWE toolset. |
| `agents/builtin/explorer.toml` | Read-only sub-agent for skills with `agent: explorer` frontmatter. |

## Test commands

```
cargo test -p hivecore-config
```

## v0.2 backlog

- Multi-provider support (`anthropic`, `ollama`, …) — currently `openai` only.
- Schema versioning (`schema_version` field) so older agent files don't silently break.
- Workflow + policy schemas (currently only agent definitions).
