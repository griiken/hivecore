# crates/hivecore-compaction/AGENTS.md

**Layer 3 — session compaction (ADR-026).** Default `ContextTransform` impl
that summarizes the head of a long conversation while keeping the tail
intact, then derives the per-turn model payload from the on-disk message
log + a `compaction_marker`.

## Why a separate crate

ADR-026 §"crate decision" — compaction is opinionated policy. Layer 2
(`hivecore-agent-loop`) must stay neutral. Hivecore has multiple harnesses
(`hivecore-coder`, `hivecore-acp-server`) that share this policy → can't
duplicate. → standalone Layer-3 crate.

Industry-pattern note: Codex / pi / jcode bundle compaction inside their
single agent crate. They're single-harness; we aren't.

## What lives here

| Module | Purpose |
|---|---|
| `constants` | `CompactionConfig` + `DEFAULT_COMPACTION_CONFIG` (jcode thresholds). |
| `marker` | `CompactionMarker` payload + `to_message()` / `try_from_message()` helpers. Marker rides `AgentMessage::Custom { kind: "compaction_marker", visible_to_model: false, payload }`. |
| `prompts` | `SUMMARY_PROMPT` (vendored pi-mono, MIT) + `SUMMARY_PREFIX` (vendored Codex, Apache-2.0). Files in `prompts/`. |
| `token` | `estimate_tokens` — chars/4 heuristic; structural cost for tool calls + images. |
| `transform` | `SummarizingTransform` — the `ContextTransform` impl. |

## How `SummarizingTransform` works

1. **`maybe_compact(state)`** is called by the driver (in
   `hivecore-agent-loop`) before each `model.complete()`.
2. Token estimate of `state.messages`. If below threshold → `Ok(None)`.
3. Above threshold → pick a cut point preserving the last
   `recent_turns_to_keep` user/assistant turns; serialise the head into
   text; call summarizer model with `SUMMARY_PROMPT`; collect summary text;
   build `CompactionMarker` (with structural `file_refs` extracted from
   tool calls in head); return `Some(marker.to_message())`.
4. Driver pushes the marker into `state.messages` so it persists to disk
   via `MessageCommitted`.
5. **`transform_outgoing(state, messages)`** runs next. Walks
   newest→oldest to find the latest marker. Replacement payload (Codex
   shape — preserves real user messages):
   `[real user msgs from head] + [synthetic-assistant(SUMMARY_PREFIX + summary)] + [kept tail]`

Multiple markers stack. Latest wins; older markers + the messages they
covered are entirely replaced by the latest summary.

## Default thresholds

```rust
context_window: 128_000        // gpt-5.4-nano shape; override per-agent
threshold: 0.80                // background fire
critical_threshold: 0.95       // emergency / synchronous
reserve_tokens: 16_384         // pi pattern
recent_turns_to_keep: 10       // tail preserved
min_turns_to_keep: 2           // floor
emergency_tool_result_max_chars: 4_000
```

All overridable per-agent under `[agent.compaction]` (TOML hookup is the
job of `hivecore-config`).

## Test commands

```
cargo test -p hivecore-compaction
cargo clippy -p hivecore-compaction --all-targets -- -D warnings

# Live e2e (small budget forces compaction quickly):
HIVECORE_SESSIONS_DIR=/tmp/c OPENAI_API_KEY=sk-... \
  cargo run -p hivecore-coder --bin hivecore-coder -- \
  --context-window 4000 --recent-turns-to-keep 2 "trigger compaction"
```

## v0.2 backlog

- **`UPDATE_SUMMARIZATION_PROMPT`** (pi pattern) — when a prior compaction
  exists, fold its summary into the new one rather than re-summarising the
  head from scratch. Pi has this; we currently don't.
- **Real tokenizer** (tiktoken-rs) — replace `chars / 4` heuristic for
  models where it lies. Hook through `last_response.usage.prompt_tokens`
  when available (pi `getLastAssistantUsage`).
- **Mid-turn `ContextWindowExceeded` retry** — adapter returns typed error
  → driver catches → fires emergency `messages.remove(0)` + retry, bounded.
- **Per-agent `summarization_model`** — Zed pattern. Agent TOML carries a
  separate cheaper model handle for summaries; propagate to subagents.
- **`MANUAL_COMPACT_MIN_THRESHOLD`** enforcement — refuse `/compact`
  user-action if ≤10% of budget consumed.
- **Tail tool-result truncation in emergency mode** — apply
  `emergency_tool_result_max_chars` not just during summarization
  serialisation but also to the kept-tail payload when in critical zone.

## Stability

Pre-1.0 — `CompactionMarker` payload schema may evolve (e.g. adding more
structural fields beyond `file_refs`). Persisted markers in user logs
must be loadable via `serde(default)` annotations on new fields; migrate
via `migrate_session_entries` at the persistence layer if a breaking
change is required.
