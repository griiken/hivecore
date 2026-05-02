# crates/hivecore-system-prompt/AGENTS.md

**Layer 3 — layered system prompt builder.** Composes the model's system prompt from N typed sources in a deterministic, cache-friendly order. Mirrors Codex's five-layer model.

## Vendored upstream prompts (Apache-2.0)

| File | Source | Used as |
|---|---|---|
| `prompts/codex-base.md` | [openai/codex `codex-rs/protocol/src/prompts/base_instructions/default.md`](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/prompts/base_instructions/default.md) | `CODEX_BASE_INSTRUCTIONS` constant — default base layer. |
| `prompts/zed-template.hbs` | [zed-industries/zed `crates/agent/src/templates/system_prompt.hbs`](https://github.com/zed-industries/zed/blob/main/crates/agent/src/templates/system_prompt.hbs) | `ZED_SYSTEM_PROMPT_TEMPLATE` constant — reference. |

Both are vendored verbatim — change only via `prompts/README.md`'s sync flow.

## What lives here

| Module | Purpose |
|---|---|
| `source` | `ContextSource` trait + `RoleHint` + `StaticSource`. |
| `environment` | `EnvironmentContextSource` — Codex `<environment_context>` block (cwd / shell / date). |
| `agents_md` | `AgentsMdSource` — re-implements Codex's AGENTS.md spec verbatim (root → cwd walk, `AGENTS.override.md` precedence, 32 KiB cap). |
| `builder` | `SystemPromptBuilder` + `SystemPromptOutput` (carries fragments + a flat `system` string). |
| `error` | `PromptError`. |

## Five-layer order

`SystemPromptBuilder::codex_default(...)` enforces:

1. **Base instructions** — agent's "who you are". Default: `CODEX_BASE_INSTRUCTIONS`.
2. **Permissions** (optional) — sandbox + approval policy text.
3. **Developer instructions** (optional) — operator-supplied.
4. **AGENTS.md aggregation** (optional) — `AgentsMdSource`.
5. **Environment context** — `EnvironmentContextSource`.

Plus the user's actual message, appended by the loop later.

## Prefix-preservation invariant

The builder always emits sources in the same order so the system prompt is a stable prefix across turns (the property required for provider prompt caching). Mid-session changes (cwd flip, sandbox flip) should be appended as new sections, not mutating earlier ones — but that's the *caller's* discipline. The builder itself is stateless.

## Test commands

```
cargo test -p hivecore-system-prompt
```

## v0.2 backlog

- `RoleHint` actually used: today the builder concatenates everything into one `system` string; harnesses targeting Responses API directly should be able to read fragments and route by role.
- `scripts/sync-prompts.sh` — automate the vendored-prompt refresh + diff-vs-upstream check.
- Permissions source: a typed `PermissionsSource` that emits Codex's `<permissions instructions>` block from a sandbox config.
