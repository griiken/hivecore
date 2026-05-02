# Vendored system prompts

These files are **verbatim copies** of upstream system prompts, used as
reference material and as the default content the layered prompt builder
ships with.

| File | Source | License | Last sync |
|---|---|---|---|
| `codex-base.md` | [openai/codex `codex-rs/protocol/src/prompts/base_instructions/default.md`](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/prompts/base_instructions/default.md) | Apache-2.0 | 2026-04-30 |
| `zed-template.hbs` | [zed-industries/zed `crates/agent/src/templates/system_prompt.hbs`](https://github.com/zed-industries/zed/blob/main/crates/agent/src/templates/system_prompt.hbs) | Apache-2.0 (this file specifically — `crates/agent_settings/LICENSE-APACHE`) | 2026-04-30 |

The Zed handlebars template uses the `{{...}}` syntax; we keep it as-is for
fidelity. When hivecore renders it, we substitute the variables via our own
templating layer (see `src/render.rs`).

To resync, run `scripts/sync-prompts.sh` (TODO).
