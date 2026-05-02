# CLAUDE.md

This file is a thin pointer for Claude Code. The single source of truth is **`AGENTS.md`** at the repo root, which all coding agents (Claude Code, Codex, Cursor, Continue, Cline, Windsurf, Aider, OpenCode) read by convention.

**Read these in order:**

1. **@AGENTS.md** — project rules, dev commands, v0.1 substrate map, behavioural guidelines, licensing, token efficiency.
2. **@docs/SESSION-START.md** — ten-minute warmup for a fresh session.
3. **@docs/architecture.md** + **@docs/concepts.md** — design fundamentals.
4. **@docs/DECISIONS.md** — index of ADRs; per-ADR files under `docs/adr/<NNN>.md` (28 ADRs as of v0.1).
5. The **`AGENTS.md`** inside any crate you're working in (under `crates/hivecore-*/AGENTS.md`) — crate-local invariants, modules, v0.2 backlog.

The vendored Codex base instructions in `crates/hivecore-system-prompt/prompts/codex-base.md` describe the **AGENTS.md spec** (root → cwd walk, `AGENTS.override.md` precedence, 32 KiB cap). Hivecore's own `crates/hivecore-system-prompt/src/agents_md.rs` re-implements that spec — meaning when a hivecore agent runs against this repo, it picks up these same `AGENTS.md` files.

## Claude-Code-only addenda

Things that apply to Claude Code specifically and don't belong in the cross-agent `AGENTS.md`:

- Use `TaskCreate` / `TaskUpdate` for work spanning three or more steps.
- Delegate research-heavy work to the `spec-writer` subagent.
- Use GSD harness (`/gsd-*` skills) for phase-driven work; `.planning/` is auto-managed by GSD.
- Research source pointers go in `.research/` (10-line max per source — bookmarks not content dumps).
