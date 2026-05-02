# AGENTS.md

Rules for any coding agent working inside this repository (Codex, Claude Code, Cursor, Continue, Cline, Windsurf, Aider, OpenCode, others). Self-contained — no external references required.

## Project

Hivecore — OSS substrate for org-customizable AI SDLC harnesses. Written in Rust (backend, CLI) and TypeScript (Next.js frontend, planned).

Public documentation lives under `docs/`. Start with `docs/introduction.md`, `docs/concepts.md`, and `docs/architecture.md`. Research source pointers live under `.research/`. Vendored prior-art summaries live under `.planning/intel/`.

## v0.1 substrate (shipped)

Crates live in a single Cargo workspace under `crates/`. Each has its own `AGENTS.md` — read that first when working in a crate.

| Crate | Layer | Purpose |
|---|---|---|
| `hivecore-runtime-core` | 1 | I/O-free traits: `Tool`, `ModelAdapter`, `EventSink`, `LifecycleHook`, `ContextTransform`, `AbortSignal`, `Approval`, `RiskAugmenter`. |
| `hivecore-openai-adapter` | 2 | Chat Completions w/ streaming + reasoning. Apache-2.0 deps only. |
| `hivecore-agent-loop` | 2 | Driver: pi-shape nested loop + `SpawnAgentTool` + lifecycle wiring. |
| `hivecore-builtin-tools` | 3 | `read_file` / `write_file` / `edit_file` / `bash` / `grep`. |
| `hivecore-persistence` | 3 | Session JSONL + ADR-019 audit JSONL. |
| `hivecore-acp-server` | 3 | Agent Client Protocol server (binary `hivecore-acp`). HITL via `session/request_permission` (ADR-029). Env opt-out: `HIVECORE_NO_APPROVAL=1`. |
| `hivecore-extension-host` | 3 | WASM Component Model loader + WIT v0.1.0. |
| `hivecore-config` | 3 | TOML `Agent` registry. (Was "Persona" — renamed for industry alignment.) |
| `hivecore-skills` | 3 | md+frontmatter + variables + `!`<cmd>`` injection + watcher + `agent:` routing. |
| `hivecore-system-prompt` | 3 | Layered prompt builder. Codex `default.md` + Zed `system_prompt.hbs` vendored verbatim. |
| `hivecore-coder` | 3 | Sample harness — CLI coding agent. Different shape than `hivecore-acp-server`; proves substrate composability (ADR-016). |
| `hivecore-compaction` | 3 | `SummarizingTransform` (ADR-026). Vendors pi summary prompt (MIT) + Codex summary-prefix preamble (Apache-2.0). Fires at 80% / 95% by default; preserves real user messages; structural file refs persist past markers. |
| `hivecore-tool-policy` | 3 | HITL approval primitives (ADR-029): `ApprovalHook: impl ToolHook`, 3-state `ToolMatcher`, session cache, `apply_exclude` static filter, `RiskAugmenter` plumbing. Wired into `hivecore-coder` (CLI prompter) and `hivecore-acp-server` (ACP prompter). |
| `hivecore-mcp-client` | 3 | Opt-in MCP integration (ADR-027). Lazy 3-meta-tool default (`mcp_servers` / `mcp_discover` / `mcp_call`) over `rmcp` stdio transport. TOML config layered user-global + workspace-local. `McpRiskAugmenter` consumes `read_only_hint` per ADR-029 A3. Not in kernel; not a WASM extension. See ADR-027 for backlog. |
| `hivecore-browser-core` | 2 | `BrowserProvider` trait + `Snapshot` / `ElementRef` / `SnapshotVersion` / `ActionKind` / `WaitCondition` / `AssertPredicate` (ADR-028). Pure, no I/O. Lifted from jcode BPP (MIT). |
| `hivecore-browser-runtime` | 3 | `ChromeProvider` over `chromiumoxide` (ADR-028). JS-walker snapshot, CDP `act` dispatch, MutationObserver settle, real `WaitCondition` predicates. Multi-tenant by construction. Validation reports under `.research/browser-*-validation.md`. |
| `hivecore-browser-harness` | 3 | Typed `Tool` impls — `browser_session` / `navigate` / `snapshot` / `act` / `wait` / `assert` / `screenshot` (ADR-028). `browser_act` is a single `kind`-enum tool covering all primitives. Live e2e examples in `examples/`. |

Unit tests + live e2e flows ship per crate. See `crates/hivecore-<name>/AGENTS.md` for per-crate entry points and `cargo test --workspace` for the live count.

## Session resume + compaction (ADR-026)

Sessions persist as append-only JSONL at `<sessions_dir>/<tenant_id>/<session_id>.jsonl`. Compaction never mutates the on-disk log — it appends a `Custom { kind: "compaction_marker", visible_to_model: false }` message. The model payload sent each turn is **derived** from disk by `ContextTransform::transform_outgoing` walking newest-first to the latest marker. Resume = replay disk → seed `AgentState.messages` → driver fires the same transform → identical shrunk payload.

Key invariants:

- **Disk is source of truth, never mutated.** Multiple compactions stack as multiple markers.
- **Real user messages preserved verbatim** (Codex pattern). Summary covers assistant + tool turns only.
- **Sub-agent transcripts** at `<sessions_dir>/<tenant_id>/<parent_id>/subagents/<child_id>.jsonl` (pi pattern).
- **Lock file** `<session_id>.jsonl.lock` is advisory; second process refuses with `--fork` hint.
- **Tenant in path** from day one; v0.1 single-tenant uses `tenant_id = "default"`.

Default thresholds (configurable per-agent under `[agent.compaction]`): `0.80` background trigger, `0.95` emergency drop, `0.10` manual-/compact floor, `recent_turns_to_keep = 10`, `min_turns_to_keep = 2`, `reserve_tokens = 16384`, `emergency_tool_result_max_chars = 4000`.

## Critical rules

- Keep core library crates I/O-free (`crates/hivecore-runtime-core/`). I/O lives in `*-adapter`, `*-runtime`, or `*-host` crates.
- No cross-crate hidden coupling; every dependency through explicit traits or exported types.
- `thiserror` for libraries, `anyhow` for binaries.
- `workspace = true` in member `Cargo.toml` for shared metadata.
- Frontend (`apps/web`, future): React Query owns server state; Zustand owns client state. WS events invalidate React Query, never write directly to stores.
- Agents (TOML), workflows, and policies are CONFIG, not code. Config schema lives in `crates/hivecore-config/`.
- KG ontology changes require an ADR before merge.
- All v0.1 crates ship `0.1.0` and are pre-1.0 — breaking changes likely until 1.0. See per-crate `AGENTS.md` for stability notes.
- True throwaway prototypes (if needed in future) go on a `spike/<topic>` *branch*, never a `spikes/` directory in main.

## Dev commands

```
cargo check --workspace
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

All four must pass before a PR is merged. Frontend commands return when `apps/web/` lands.

## Workflow

- Docs before code for non-trivial design.
- One logical change per commit and per PR.
- Branches: `feature/<scope>`, `fix/<scope>`, `docs/<scope>`, `rfc/<name>`, `spike/<topic>`.
- Commits in imperative mood.
- Update `CHANGELOG.md` for user-visible changes.
- Non-trivial proposals go through an `rfc:`-labelled issue with one-week comment period before implementation PR.
- TDD: spike → prove (scenario passes) → harden → tests → docs → attach to main.

## Communication

Match the project lead's tone. Keep technical substance exact.

## Behavioural guidelines

Reduce common LLM coding mistakes. Bias toward caution over speed; use judgement on trivial tasks. Adapted from Karpathy's working set. Worked Rust + hivecore-shaped examples for each rule live in [`docs/EXAMPLES.md`](./docs/EXAMPLES.md).

### 1. Think before coding

Don't assume. Don't hide confusion. Surface tradeoffs.

- State assumptions explicitly. If uncertain, ask.
- Multiple interpretations exist → present them. Do not pick silently.
- Simpler approach available → say so. Push back when warranted.
- Unclear → stop. Name what's confusing. Ask.

### 2. Simplicity first

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" / "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- 200 lines that could be 50 → rewrite.

Test: would a senior engineer call this overcomplicated? If yes, simplify.

### 3. Surgical changes

Touch only what you must. Clean up only your own mess.

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor what isn't broken.
- Match existing style even if you'd do it differently.
- Notice unrelated dead code → mention it, don't delete it.

When your changes create orphans:
- Remove imports / variables / functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

Test: every changed line traces directly to the user's request.

### 4. Goal-driven execution

Define success criteria. Loop until verified.

- "Add validation" → write tests for invalid inputs, then make them pass.
- "Fix the bug" → write a test that reproduces it, then make it pass.
- "Refactor X" → ensure tests pass before and after.

For multi-step tasks, state a brief plan:

```
1. [step] → verify: [check]
2. [step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") force constant clarification.

Working if: fewer unnecessary changes in diffs, fewer rewrites from overcomplication, clarifying questions come before implementation, not after.

## Licensing

Apache License 2.0 (see ADR-030 — superseded ADR-002's dual MIT-or-Apache-2.0 choice on 2026-05-03). New source files need no per-file licence header; the root `LICENSE` covers the tree.

**Vendored upstream prompts under `crates/hivecore-system-prompt/prompts/` are Apache-2.0** (Codex / Zed) — same licence, no compatibility friction. The `prompts/README.md` documents source URLs + sync date.

## Token Efficiency

- Never re-read files you just wrote or edited.
- Never re-run commands to "verify" unless the outcome was uncertain.
- Don't echo large blocks of code or file contents unless asked.
- Batch related edits into single operations.
- Skip confirmations. Just do it.
- If a task needs 1 tool call, don't use 3.
- Do not summarise what you just did unless the result is ambiguous.


<claude-mem-context>
# Memory Context

# [hivecore] recent context, 2026-05-01 8:19pm GMT+5:30

No previous sessions found.
</claude-mem-context>