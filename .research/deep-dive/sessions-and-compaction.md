# Deep dive — session resume + compaction

Direct read of OSS source at HEAD on 2026-05-01. Files cached at `/tmp/hcr-deepdive/*.txt`.

## Codex (`openai/codex`, Rust)

### File map
- `codex-rs/core/src/compact.rs` (560 lines) — algorithm
- `codex-rs/core/templates/compact/prompt.md` — summarization prompt
- `codex-rs/core/templates/compact/summary_prefix.md` — preamble injected ahead of summary
- `codex-rs/rollout/src/policy.rs` (90 lines) — what gets persisted
- `codex-rs/rollout/src/recorder.rs` (580 lines) — JSONL writer
- `codex-rs/rollout/src/session_index.rs` (263 lines) — name index
- `codex-rs/core/src/session/rollout_reconstruction.rs` (97 lines) — replay

### Constants
```rust
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;
```
Single per-message cap; everything else (auto-compact threshold, etc.) is **provider-config**, not a fixed constant in the source.

### Trigger paths (two)

```rust
// Manual — /compact slash command. No init-context re-injection.
async fn run_compact_task(sess, turn_context, input) -> CodexResult<()> {
    sess.send_event(TurnStarted{...}).await;
    run_compact_task_inner(
        sess, turn_context, input,
        InitialContextInjection::DoNotInject,
        CompactionTrigger::Manual,
        CompactionReason::UserRequested,
        CompactionPhase::StandaloneTurn,
    ).await
}

// Auto — fired by turn driver at threshold or mid-turn ContextWindowExceeded.
async fn run_inline_auto_compact_task(sess, turn_context, initial_context_injection, reason, phase) {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text { text: prompt, text_elements: Vec::new() }];
    run_compact_task_inner(
        sess, turn_context, input,
        initial_context_injection,            // BeforeLastUserMessage for mid-turn
        CompactionTrigger::Auto,
        reason,                                // TokenThreshold | ContextExceeded | ...
        phase,
    ).await
}
```

### Algorithm
```rust
fn build_compacted_history(
    initial_context: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
) -> Vec<ResponseItem> {
    build_compacted_history_with_limit(
        initial_context, user_messages, summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS,
    )
}
```
**Recipe:** `[init_context?] + truncated(user_messages, 20k each) + assistant(summary_text)`. Everything assistant + tool gone.

### Mid-turn fallback
```rust
Err(e @ CodexErr::ContextWindowExceeded) => {
    // Trim from the beginning to preserve cache (prefix-based) and keep recent messages intact.
    history.remove_first_item();
    // retry...
}
```
Drop oldest one item at a time, retry, increment `truncated_count`, surface event "Trimmed N older items".

### Compaction prompt (verbatim)
```
You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary
for another LLM that will resume the task.

Include:
- Current progress and key decisions made
- Important context, constraints, or user preferences
- What remains to be done (clear next steps)
- Any critical data, examples, or references needed to continue

Be concise, structured, and focused on helping the next LLM seamlessly continue the work.
```

### Summary prefix (verbatim)
```
Another language model started to solve this problem and produced a summary of
its thinking process. You also have access to the state of the tools that were
used by that language model. Use this to build on the work that has already
been done and avoid duplicating work. Here is the summary produced by the other
language model, use the information in this summary to assist with your own
analysis:
```

### Session index — append-only, scan-from-end
File: `~/.codex/session_index.jsonl`. Schema:
```rust
pub struct SessionIndexEntry {
    pub id: ThreadId,
    pub thread_name: String,
    pub updated_at: String, // RFC3339
}
const SESSION_INDEX_FILE: &str = "session_index.jsonl";
```
- `append_thread_name(codex_home, thread_id, name)` — every rename appends a line. Most recent line wins on lookup.
- `find_thread_name_by_id` — `scan_index_from_end_by_id` (8 KiB read chunks). Linear scan from EOF.
- `find_thread_names_by_ids` (batch) — bulk lookup over a `HashSet<ThreadId>`.

Two index files total: `session_index.jsonl` (id ↔ name) + the per-session rollouts under `~/.codex/sessions/rollout-*-*.jsonl`.

### Persistence policy
```rust
pub fn should_persist_event_msg(ev: &EventMsg, mode: EventPersistenceMode) -> bool {
    match mode {
        EventPersistenceMode::Limited => should_persist_event_msg_limited(ev),
        EventPersistenceMode::Extended => should_persist_event_msg_extended(ev),
    }
}
```
Two modes: `Limited` (default — only "interesting" UX events) and `Extended` (everything). `SessionMeta`, `TurnContext`, `Compacted` always persisted regardless — they're **structural** markers needed for reconstruction.

---

## Pi (`badlogic/pi-mono`, TypeScript)

### File map
- `packages/coding-agent/src/core/compaction/compaction.ts` (839 lines)
- `packages/coding-agent/src/core/compaction/branch-summarization.ts` (125 lines) — sub-agent fork
- `packages/coding-agent/src/core/session-manager.ts` (1425 lines) — session state I/O

### Pipeline (12 staged exports)
```
calculateContextTokens     - estimate
getLastAssistantUsage      - read provider usage from last response
estimateContextTokens      - heuristic if no usage
shouldCompact              - threshold check
findTurnStartIndex         - locate turn boundaries
findCutPoint (CutPointResult) - decide where to slice
generateSummary            - call model with SUMMARIZATION_PROMPT
prepareCompaction (CompactionPreparation)
compact                    - apply
```
**Different shape than Codex.** Pi finds a *cut point* and summarises only the **head** — keeps the **tail** of messages intact. Codex throws away assistant+tool entirely. Pi's `RECENT_TURNS_TO_KEEP`-style preservation is finer-grained.

### Compact entry point
```ts
export async function compact(
    preparation: CompactionPreparation,
    model: Model<any>,
    apiKey: string,
    headers?: Record<string, string>,
    customInstructions?: string,
    signal?: AbortSignal,
    thinkingLevel?: ThinkingLevel,
    ...
)
```
`customInstructions` lets harness override the default prompt.

### Summarization prompt (structured Markdown — the strictest of the four)
```
The messages above are a conversation to summarize. Create a structured context
checkpoint summary that another LLM will use to continue the work.

Use this EXACT format:

## Goal
[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned by user]
- [Or "(none)" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Current work]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [Ordered list of what should happen next]

## Critical Context
- [Any data, examples, or references needed to continue]
- [Or "(none)" if not applicable]

Keep each section concise. Preserve exact file paths, function names, and error messages.
```

### Branch (sub-agent fork) summary
Same structure with section heads `## Goal`, `## Progress > Done`, etc. but framed as "sub-agent finished its branch" rather than "main session continues". Output: `BranchSummaryEntry`.

### SessionEntry discriminated union (rich!)
```ts
SessionMessageEntry        // user/assistant turns
ThinkingLevelChangeEntry   // mid-session thinking-effort change
ModelChangeEntry           // mid-session model swap
CompactionEntry<T = unknown>  // generic — extensions carry typed payload
BranchSummaryEntry         // sub-agent fork output
CustomEntry                // extension-defined
LabelEntry                 // session label/title
SessionInfoEntry           // metadata
CustomMessageEntry         // arbitrary message-shaped data
FileEntry                  // file reference (filename + content)
```
- `CompactionEntry` generic over `T` — orgs can add structural fields (Pi's main wedge).
- `getLatestCompactionEntry(entries)` — walks back to find the most recent. **Same shape as Codex's `Compacted` marker scan, but TS-side.**
- `findMostRecentSession` for `--continue`.
- `migrateSessionEntries` — version-tolerant load.
- `firstKeptEntryIndex` field on `CompactionEntry` — exact int boundary index. Cleaner than Codex's marker-only approach.

### Resume
```ts
/** Switch to a different session file (used for resume and branching) */
```
One method serves both `--resume` and fork. `SessionTreeNode` keeps the parent/child topology; `SessionManager` exposes a `ReadonlySessionManager` view for branching agents.

---

## Zed (`zed-industries/zed`, Rust)

### File map
- `crates/agent/src/db.rs` (sqlez over SQLite — not JSONL)
- `crates/agent/src/thread.rs` (4596 lines — bulk of compaction logic)

### Storage = SQLite, not JSONL
```rust
pub struct DbThread {
    pub title: SharedString,
    pub messages: Vec<DbMessage>,
    pub updated_at: DateTime<Utc>,
    pub detailed_summary: Option<SharedString>,                        // ← additive metadata
    pub initial_project_snapshot: Option<Arc<crate::ProjectSnapshot>>,
    pub cumulative_token_usage: language_model::TokenUsage,            // ← global counter
    pub request_token_usage: HashMap<acp_thread::UserMessageId, language_model::TokenUsage>,  // ← per-msg
    pub model: Option<DbLanguageModel>,
    pub profile: Option<AgentProfileId>,
    // … draft_prompt, subagent_context, speed, thinking_enabled, thinking_effort
}

pub struct DbThreadMetadata {
    pub id: acp::SessionId,
    pub parent_session_id: Option<acp::SessionId>,                     // ← fork link
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub folder_paths: PathList,                                        // ← grouped by workspace
}
```
**Summary is metadata, not a replacement.** Compaction in Zed adds a `detailed_summary` field; the message log itself stays full. Different philosophy from Codex/Pi.

### Two-prompt setup (`SUMMARIZE_THREAD_PROMPT` short, `SUMMARIZE_THREAD_DETAILED_PROMPT` long)
Prompt strings live in `agent_settings`, not in `thread.rs`. Short prompt for thread titles / quick recap; detailed for handoff/persistence.

### Cheap-summarization-model
```rust
pub fn summarization_model(&self) -> Option<&Arc<dyn LanguageModel>> { self.summarization_model.as_ref() }
self.summarization_model = parent.summarization_model.clone();   // propagated to subagent
let subagent_summary_id = subagent.read(cx).summarization_model().unwrap().id();
"Subagent summarization model should match parent after set_summarization_model"
```
Separate model handle. Propagates from main thread → subagent. **No fixed auto-compact threshold visible**; user-triggered.

---

## jcode (`1jehuang/jcode`, Rust)

### Constants — most explicit of the four
```rust
const DEFAULT_TOKEN_BUDGET: usize = 200_000;          // matches Claude
const COMPACTION_THRESHOLD: f32 = 0.80;               // background trigger
const CRITICAL_THRESHOLD: f32 = 0.95;                 // emergency drop-oldest
const MANUAL_COMPACT_MIN_THRESHOLD: f32 = 0.10;       // refuse manual /compact under 10%
const RECENT_TURNS_TO_KEEP: usize = 10;               // tail preserved by background path
const MIN_TURNS_TO_KEEP: usize = 2;                   // floor — never strip below 2
const EMERGENCY_TOOL_RESULT_MAX_CHARS: usize = 4000;  // truncate tool results in emergency
const CHARS_PER_TOKEN: usize = 4;                     // crude estimator
const SYSTEM_OVERHEAD_TOKENS: usize = 18_000;         // assumed system-prompt cost
const TOKEN_HISTORY_WINDOW: usize = 20;               // EWMA proactive window
const EMBED_MAX_CHARS_PER_MSG: usize = 512;
const EMBEDDING_HISTORY_WINDOW: usize = 10;
```

### Pattern: tail preserved
`RECENT_TURNS_TO_KEEP = 10`, `MIN_TURNS_TO_KEEP = 2` — same shape as pi's "summarize head, keep tail". Different from Codex (drops everything assistant+tool).

### Embedded summary prompt
```
Summarize our conversation so you can continue this work later.
Write in natural language with these sections:
- **Context:** What we're working on and why (1-2 sentences)
- **What we did:** Key actions taken, files changed, problems solved
- **Current state:** What works, what's broken, what's next
- **User preferences:** Specific requirements or decisions they made
Be concise but preserve important details. You can search the full conversation
later if you need exact error messages or code snippets.
```
Less structured than pi's, more structured than Codex's.

### Three modes
- **Reactive** — fixed 80% threshold, fires when crossed
- **Proactive** — EWMA over `TOKEN_HISTORY_WINDOW = 20` turns, predicts overflow, compacts early
- **Semantic** — embeddings (`all-MiniLM-L6-v2`) detect topic shift; falls back to proactive if embed unavailable

### Emergency hard-compact
Above 95%, synchronously drop oldest items WITHOUT summary. `EMERGENCY_TOOL_RESULT_MAX_CHARS = 4000` truncates tool-result blobs in place. "Data loss preferred to API failure."

---

## Warp (`warpdotdev/warp`, Rust, AGPL-3.0 + MIT)

**License caution.** Repo is AGPL-3.0 (Rust crates) with MIT for some modules. AGPL-3.0 is **incompatible** with hivecore's MIT-OR-Apache-2.0 — we **cannot vendor** Warp source verbatim (would force the entire downstream into AGPL). Read for patterns only; reimplement.

### File map (relevant)
- `app/src/ai/agent/conversation.rs` (3741 lines) — main conversation state
- `app/src/ai/restored_conversations.rs` (61 lines) — singleton holding restored sessions at app startup
- `app/src/ai/persisted_workspace.rs` (1257 lines) — persistence enums (SQLite-tied)
- `app/src/ai_assistant/transcript.rs` (979 lines) — transcript view + summarization status
- `app/src/ai/conversation_navigation/mod.rs` (348 lines) — restored-conversation navigation
- `app/src/ai/agent/linearization.rs` — turn-tree → flat list
- `crates/ai/src/agent/{action,action_result,citation.rs,convert.rs,file_locations.rs}` — action enum (ADR-018 inspiration)

### Conversation state
```rust
// conversation.rs:147
/// Usage metadata for this conversation, including summarization status, context window usage,
/// model token usage, tool usage, ...

pub fn was_summarized(&self) -> bool {
    self.conversation_usage_metadata.was_summarized
}
pub fn context_window_usage(&self) -> f32 {
    self.conversation_usage_metadata.context_window_usage
}
// "A conversation can never go from summarized to un-summarized" — invariant comment at line 1617
```
Summarization is a **boolean flag + metadata**, not a history replacement. **Same as Zed.**

### Truncation API
```rust
// conversation.rs:3250
/// Truncates the conversation from the given exchange ID, removing all exchanges
/// after that point.
pub fn truncate_from_exchange(
    &mut self,
    exchange_id: ...,
) -> ... {
    let truncate_from_idx = all_exchanges.iter().position(|e| e.id == exchange_id)?;
    all_exchanges[truncate_from_idx..].iter().copied().collect();
    // ...
}
```
**Forward-truncation only.** User picks an exchange, everything after drops. No auto-truncation found.

### Persistence
```rust
// persisted_workspace.rs:54
/// This is also used in underlying sqlite type persistence. We should be careful
/// not to rename an existing variant, as it will break persistence.
```
**SQLite, not JSONL.** Same as Zed. Enum variants are schema-tied — backward-compat baked in.

### Restore on startup
```rust
// restored_conversations.rs
/// Singleton model that holds restored agent conversations on app startup.
/// Loading restored conversations into this model is a means of propagating restored data from
/// the persistence layer to ...
pub struct RestoredAgentConversations { ... }

// transcript.rs / conversation_navigation:
//   convert_persisted_conversation_to_ai_conversation_with_metadata(conversation)
```
**Pattern: load all on startup as a singleton, then route by `conversation_id`.** Different from Codex (lazy load by id) and pi (load latest by cwd).

### What Warp doesn't have (visible in source)
- No fixed token threshold.
- No auto-summarization trigger that I could find — appears user-driven.
- No structured-section summary prompt visible. Likely lives in cloud agent template, not in repo.
- No mid-turn `ContextWindowExceeded` fallback found.
- Explicit `was_summarized` invariant — once flagged, can't unflag. Persistence-friendly.

---

## Synthesis — five tools, one decision tree

| Decision | Codex | Pi | jcode | Zed | Claude Code |
|---|---|---|---|---|---|
| Storage | JSONL | JSONL | JSONL | SQLite | JSONL |
| (Warp also: SQLite + singleton-restore) | | | | | |
| Index file | `session_index.jsonl` (append-only, scan-from-end) | session-manager-owned | `metadata_requires_snapshot` | sqlite query | `~/.claude/projects/<slug>/` directory listing |
| Compaction shape | replace history: `[user_msgs..., summary]` | replace head, keep tail | replace head, keep tail (10 recent) | summary as metadata, log untouched | replace history (compact_boundary line) |
| Threshold | provider-config (no fixed) | configurable | 80% / 95% explicit | none (manual only) | 95% (env-overrideable) |
| Mid-turn OOM | `remove_first_item()` retry | n/a | drop oldest, no summary | n/a | n/a |
| Cheap summary model | no | no | yes (codex spark sidecar) | yes (`summarization_model`, propagates) | unknown |
| Sub-agent | `Compacted` marker w/ phase | `branch-summarization.ts` | own modes | `parent_session_id` + propagated model | own JSONL `subagents/agent-{id}.jsonl` |
| Structural payload | none (text summary only) | `CompactionEntry<T>` generic — orgs add fields | `summarized_messages` count, `covers_up_to_turn` | `cumulative_token_usage` per-msg + global | `compact_file_reference`, `invoked_skills`, etc. |
| Resume API | `/resume` slash, `--continue` | `findMostRecentSession` + `switch session file` | `find_session_by_name_or_id`, crash recovery | sqlite by `acp::SessionId` | `-c` / `-r <name|id>` |

## Recommendation refined for hivecore

### Storage
- **JSONL.** SQLite (Zed) trades simplicity for queryability we don't need yet.
- One file per session: `<sessions_dir>/<RFC3339>-<uuid>.jsonl` (Codex shape).
- Plus **append-only `session_index.jsonl`** (Codex shape, exactly): id + name + updated_at, scan-from-end on lookup. Cheaper than directory listing + handles renames.

### Compaction shape — **pick pi's, not Codex's**
Codex's "drop everything assistant+tool" is aggressive. Pi/jcode keep a tail (`RECENT_TURNS_TO_KEEP = 10`). For an SDLC harness where recent tool results matter (file edits in flight), tail preservation is the right default.

```rust
// proposed
struct Compacted {
    summary_text: String,
    covers_up_to_turn: TurnId,           // pi: firstKeptEntryIndex
    original_turn_count: usize,
    kept_tail_turns: usize,              // pi: RECENT_TURNS_TO_KEEP
    file_refs: Vec<FileRef>,             // pi: readFiles + modifiedFiles
    tool_calls_summarized: usize,
    compaction_trigger: CompactionTrigger,  // Manual | Auto | Emergency
}
```

### Thresholds (jcode's exact numbers)
```rust
const COMPACTION_THRESHOLD: f32 = 0.80;
const CRITICAL_THRESHOLD: f32 = 0.95;
const MANUAL_COMPACT_MIN_THRESHOLD: f32 = 0.10;
const RECENT_TURNS_TO_KEEP: usize = 10;
const MIN_TURNS_TO_KEEP: usize = 2;
```
Make them per-agent overridable (TOML).

### Summarization prompt — pi's structured form
Strict markdown sections. Easier for downstream KG ingest than Codex's free-form. Vendor it the same way we vendored Codex base instructions (Apache-2.0; pi-mono is MIT, both compatible).

### Cheap-summary model — Zed's pattern
Add `[agent.compaction] summarization_model = "gpt-5.4-nano"` per-agent, default to the chat model.

### Resume CLI (Claude Code's CLI shape, Codex's lookup mechanics)
```
hivecore-coder --continue              # latest by cwd
hivecore-coder --session <id|name>     # explicit
hivecore-coder --name auth-refactor    # set name (writes to session_index.jsonl)
```

### Emergency drop-oldest (Codex + jcode)
On `ContextWindowExceeded` mid-call: `messages.remove(0)`, retry, surface event. Above 95%: synchronous drop-oldest without summarization. Match Codex's comment: *"Trim from the beginning to preserve cache (prefix-based)."*

### Persistence policy — Codex's two-mode shape
- `Limited` (default) — interesting events only
- `Extended` (env-flag) — everything
Always-persisted set: session-meta, turn-context, compacted, audit-plane events.

---

## Crates touched (final)

| Crate | Change |
|---|---|
| `hivecore-runtime-core` | nothing — `ContextTransform`, `LifecycleHook`, `Custom` already there |
| `hivecore-agent-loop` | wire `transform_outgoing` before each `model.complete`; add `Builder::resume(messages)` + `Builder::context_transform(...)` |
| `hivecore-persistence` | add `Compacted` entry kind to `SessionEntry`; add `session_index.jsonl` w/ scan-from-end (Codex pattern); add `reconstruct_from_jsonl` |
| `hivecore-compaction` (NEW) | `SummarizingTransform` (pi-shape: head→summary, keep tail), `TokenThresholdHook` (jcode constants), `EmergencyDropHook`, default prompt vendored from pi w/ Apache-compatible attribution |
| `hivecore-coder` | `--continue` / `--session` / `--name` flags; wire `SessionWriter` as default sink |

Total ~600 LOC across 5 crates. Phase A (resume) ~150 LOC, Phase B (compaction) ~350 LOC, Phase C (structural observer) ~100 LOC.
