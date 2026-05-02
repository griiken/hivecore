# crates/hivecore-skills/AGENTS.md

**Layer 3 — markdown skills.** Claude-Code-shape `SKILL.md` files become `hivecore_runtime_core::Tool`s.

## Why tool-shaped

A skill is just "a tool whose body is markdown the model sees in its tool result". Description goes to the model via the standard tool list; full body loads only on call (the lazy-load Claude Code documents). No special prompt-injection plumbing required — the existing `Tool` trait is rich enough.

## What lives here

| Module | Purpose |
|---|---|
| `frontmatter` | Splits markdown into `(yaml, body)`. CRLF + BOM tolerant. |
| `skill` | `Skill` + `SkillFrontmatter` (validated record). |
| `loader` | `SkillLoader` — walks dirs for `*.md`, validates frontmatter. |
| `render` | Variable expansion (`$ARGUMENTS`, `$0..$N`, `$name`, `${SESSION_ID}`, `${SKILL_DIR}`, `${EFFORT}`) + `!`<cmd>`` bash injection (timeout + error propagation). |
| `registry` | `SkillRegistry` — uniqueness + `model_invokable_tools(...)` builder. |
| `tool` | `SkillTool` (impls `Tool`) + `SubAgentRouter` trait for `agent:` routing. |
| `watcher` | `SkillWatcher` — debounced file watcher; live reload. |

## Frontmatter fields supported

| Field | Effect |
|---|---|
| `name` | Tool name. |
| `description` | Tool description visible to the model. |
| `arguments` | Named positional args. Index N ↔ `$N` ↔ `$ARGUMENTS[N]` ↔ `$<name>`. |
| `disable-model-invocation` | Hides the tool from the model (user-only path). |
| `user-invocable` | (parsed; harness uses for slash-menu UI). |
| `allowed-tools` | (parsed; harness uses for policy). |
| `agent` | Route the rendered body into a sub-agent via `SubAgentRouter`. |
| `context: fork` | (parsed; future fork-without-named-subagent). |

## `agent:` routing

If a skill has `agent: <name>` and the registry was built with a `SubAgentRouter`, the rendered body is handed to the router (which spawns a sub-agent under that name) and the *sub-agent's final reply* becomes the tool result. Mirrors Claude Code's pattern verbatim.

The router is wired in `hivecore-acp-server` via `PersonaRouter` (uses `hivecore-config`'s `AgentRegistry`).

## Variable / bash-injection rules

- `$N` and `$<name>` references that don't resolve are left **literal** so bash injections like `awk '{print $1}'` survive.
- `!`<cmd>`` runs in the workspace root with a 30 s timeout.

## Test commands

```
cargo test -p hivecore-skills
OPENAI_API_KEY=... cargo run --example use_skill
```

## v0.2 backlog

- `context: fork` without a named sub-agent (forked context with the *current* agent).
- Description-budget cap (Claude Code truncates descriptions to ~1.5 KiB to keep the tool list compact).
- Plugin bundling (`<plugin>/skills/*` discovery + namespace).
