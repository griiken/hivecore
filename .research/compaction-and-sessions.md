# Session log + resume + compaction — prior art

Research date: 2026-05-01. Sources read at HEAD on that day. Citations are `repo:path:line` where line numbers were observed; otherwise `repo:path`.

## Per-tool findings

### Codex (`openai/codex`, Rust, OSS)

- **Log format.** JSONL, one line per event. File comment in `codex-rs/rollout/src/recorder.rs` shows the canonical path:
  `~/.codex/sessions/rollout-<RFC3339-timestamp>-<thread-uuid>.jsonl`
  Records are typed `RolloutLine { ... RolloutItem }` where `RolloutItem` is one of:
  - `SessionMeta` (header — thread id, originator, git info, base instructions)
  - `ResponseItem` (model-side: `Message`, `Reasoning`, `LocalShellCall`, `FunctionCall`, `ToolSearchCall`, `CustomToolCallOutput`, `WebSearchCall`, `ImageGenerationCall`, `Compaction`)
  - `EventMsg` (UX-side; only "interesting" ones persisted under default `Limited` policy)
  - `TurnContext` (per-turn config snapshot — model, instructions, cwd, sandbox)
  - `Compacted` (executive marker — the moment a compaction happened)
  Persistence is gated by `is_persisted_response_item` / `should_persist_event_msg` in `codex-rs/rollout/src/policy.rs`. Two modes: `Limited` (default) and `Extended`. The `SessionMeta`, `TurnContext`, `Compacted` markers are *always* persisted — they carry the structural signal needed to reconstruct.

- **Resume.** Slash menu has `Resume`, `Fork`, `New` (`codex-rs/tui/src/slash_command.rs`). On startup (or `/resume`), `RolloutRecorder::list_*` enumerates rollout files, opens the chosen one, then calls `Session::reconstruct_history_from_rollout` (`codex-rs/core/src/session/rollout_reconstruction.rs`). Reconstruction scans **newest-to-oldest**, finds the most recent `Compacted` checkpoint that survives, then forward-replays only the buffered surviving tail. The replay yields a `RolloutReconstruction { history, previous_turn_settings, reference_context_item }` — i.e. reusable history plus enough metadata to rehydrate sub-agent and `TurnContext` state.

- **Compaction trigger.** Three trigger types (`codex-rs/core/src/compact.rs`):
  - `Manual` — user runs `/compact`. Calls `run_compact_task` directly.
  - `Auto` — `run_inline_auto_compact_task` is invoked from the turn driver when the running token count crosses the model's auto-compact threshold (separate `auto_compact_token_limit` config; the actual numeric default lives in model-provider config, not in `compact.rs`).
  - `Mid-turn` — fired by the turn driver when an in-flight turn hits `ContextWindowExceeded`.
  Tracked through `CompactionTrigger { Manual | Auto }` × `CompactionReason { UserRequested | TokenThreshold | ContextExceeded | ... }` in `codex-analytics`.

- **Compaction algorithm** (`compact.rs`, function `run_compact_task_inner_impl`):
  1. Clone current history; append the user input (if any) as a normal turn item.
  2. Build a `Prompt` with the cloned history and **a fixed summarization system instruction** (`SUMMARIZATION_PROMPT` = `templates/compact/prompt.md`, contents:
     `"You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task. Include: progress, decisions, constraints, what remains, critical data."`).
  3. Stream a model response. If it returns `ContextWindowExceeded`, drop the **oldest** history item (`history.remove_first_item()`) — comment is explicit: *"Trim from the beginning to preserve cache (prefix-based) and keep recent messages intact."* — and retry. Increments `truncated_count` and surfaces a background event "Trimmed N older items".
  4. On success, build the replacement summary text as:
     ```
     {SUMMARY_PREFIX}\n{last_assistant_message_from_summary_turn}
     ```
     where `SUMMARY_PREFIX` is `templates/compact/summary_prefix.md`.
  5. `build_compacted_history(Vec::new(), &user_messages, &summary_text)` — the new history is **empty base** + collected raw user messages + the synthesized summary as an assistant-style item. So user turns are preserved verbatim; assistant + tool turns are gone, replaced by the summary.
  6. `InitialContextInjection::BeforeLastUserMessage` (mid-turn) re-injects initial context (CWD, env, base instructions) above the last real user message; `DoNotInject` (manual / pre-turn) clears it for next-turn rebuild.
  7. A `RolloutItem::Compacted(...)` marker is appended to the rollout — this is what `rollout_reconstruction.rs` keys off later.
  - `COMPACT_USER_MESSAGE_MAX_TOKENS = 20_000` caps any single user message before it's fed to the summarizer (uses `truncate_text` from `codex-utils-output-truncation`).

- **Compaction output shape.** Replaces history wholesale with `[user_messages..., summary_assistant_message]`, optionally re-prefixed with initial context. Tool call results are NOT preserved verbatim — they're folded into the summary text. Files modified are *not* tracked structurally; the model is asked to mention them in the summary.

### Claude Code (closed binary)

Source not available; from `docs.claude.com/en/docs/claude-code/*`.

- **Log format.** JSONL files at `~/.claude/projects/{project-slug}/{sessionId}/`. Main conversation is one JSONL; subagents get their own at `~/.claude/projects/{project}/{sessionId}/subagents/agent-{agentId}.jsonl` (subagent transcripts persist *independently* — main-thread compaction does not touch them). Sample line (from sub-agents docs):
  ```json
  {"type": "system", "subtype": "compact_boundary",
   "compactMetadata": {"trigger": "auto", "preTokens": 167189}}
  ```
  So events are tagged objects: `type` + `subtype` + payload.

- **Resume.**
  - `claude -c` / `claude --continue` — pick up the most recent session in the current directory.
  - `claude -r <session>` / `claude --resume <session>` — resume by id or by user-assigned name (e.g. `claude --resume auth-refactor`).
  - `/resume` interactive picker inside an active session (Ctrl+R rename, `/` filter, Ctrl+A all-projects, Ctrl+W all-worktrees, Ctrl+B current-branch).
  - Session names are first-class: `/rename auth-refactor` sets a stable handle.
  - Forks (sessions started with `--fork-session`) are grouped under root in the picker.

- **Compaction trigger.**
  - `/compact` — manual.
  - `/clear` — wipe context entirely (no summary).
  - **Auto-compaction** — triggers at ~95% of context capacity by default. Override with env `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50` to fire earlier. Same logic applies to subagents (per-subagent autocompact).

- **Compaction algorithm.** Closed; observable behavior: a `compact_boundary` system line is written to the JSONL; subsequent lines reference the post-compact state. `preTokens` is logged. Anthropic doesn't publish the exact summarization prompt or whether tool results are preserved.

- **Compaction output shape.** A boundary marker line in the JSONL; the in-memory transcript is replaced by a summary + recent tail (inferred — Anthropic doesn't document the exact algorithm).

### jcode (`1jehuang/jcode`, Rust, OSS)

- **Log format.** Per-session JSONL journal under per-session directories. Crate `jcode-session-types` exposes `SessionStatus`, `EnvSnapshot`, `GitState`, `SessionImproveMode`. The journal record is `SessionJournalEntry { meta: SessionJournalMeta, append_messages: Vec<StoredMessage>, append_env_snapshots, append_memory_injections, append_replay_events }` (`src/session/journal.rs`). `SessionJournalMeta` carries: `parent_id, title, updated_at, compaction: Option<StoredCompactionState>, provider_session_id, provider_key, model, subagent_model, working_dir, status, last_pid, saved`. Entries append; full snapshot rewritten only when metadata diverges (`metadata_requires_snapshot`).
- `StoredMessage { id, role, content: Vec<ContentBlock>, display_role, timestamp, tool_duration_ms }` (`src/session/model.rs`); `StoredCompactionState { summary_text, openai_encrypted_content, covers_up_to_turn, original_turn_count, compacted_count }`.
- `StoredReplayEvent` carries non-conversation UI events (memory injections, plan updates, swarm notices) so the replay engine in `src/replay.rs` can reproduce the session in real time (`TimelineEvent { t_ms, kind }`).
- Notable: jcode's README claims "**Resume sessions from different harnesses**. Claude code broke on you? Resume the session from jcode and continue where you left off. Session resume is supported for codex, claude code, opencode, and pi." — i.e. jcode reads other tools' on-disk JSONL formats.

- **Resume.** Crash-recovery first-class: `src/session/crash.rs` exports `detect_crashed_sessions`, `find_recent_crashed_sessions`, `find_session_by_name_or_id`, `recover_crashed_sessions`. Active session PIDs tracked in `src/session_active_pids.rs`. Session id is generated via `extract_session_name`/`new_memorable_session_id`.

- **Compaction trigger** (`src/compaction.rs`):
  ```rust
  const DEFAULT_TOKEN_BUDGET: usize = 200_000;       // matches Claude
  const COMPACTION_THRESHOLD: f32 = 0.80;            // background trigger
  const CRITICAL_THRESHOLD: f32 = 0.95;              // emergency hard-compact
  ```
  Three modes: `Reactive` (default, fixed 80% threshold), `Proactive` (EWMA token-growth-rate prediction — compacts early if predicted to overflow), `Semantic` (embedding-detected topic shift; falls back to proactive without embeddings). `ensure_context_fits` returns `CompactionAction::{ None | BackgroundStarted { trigger } | HardCompacted(usize) }`.

- **Compaction algorithm.** `CompactionManager` does **not** own messages — caller passes `&[Message]`. Manager tracks `compacted_count` (number of leading messages summarized; skipped when building API payload). Background pathway: spawn a `JoinHandle` running summarization while user keeps chatting; on completion swap in `CompactionResult { summary_text, openai_encrypted_content, covers_up_to_turn, duration_ms, summarized_messages }`. Emergency pathway (>95%): synchronously drop oldest messages, no summary — data loss preferred to API failure.
  Summary prompt is embedded:
  ```text
  Summarize our conversation so you can continue this work later.
  Write in natural language with these sections:
  - **Context:** What we're working on and why (1-2 sentences)
  - **What we did:** Key actions taken, files changed, problems solved
  - **Current state:** What works, what's broken, what's next
  - **User preferences:** Specific requirements or decisions they made
  Be concise but preserve important details. You can search the full conversation later if you need exact error messages or code snippets.
  ```
  Semantic mode constants: `EMBED_MAX_CHARS_PER_MSG = 512`, `EMBEDDING_HISTORY_WINDOW = 10`, `SEMANTIC_EMBED_CACHE_CAPACITY = 256`.

- **Compaction output shape.** `Summary { text, openai_encrypted_content, covers_up_to_turn, original_turn_count }`. Manager records `compacted_count`; downstream context builder *skips* the first N messages and prepends the summary as a synthetic message (note `SESSION_CONTEXT_PREFIX = "<system-reminder>\n# Session Context"` is used to wrap injected context). Messages on disk are NOT deleted — only the active payload changes. Replay can still see the full pre-compaction history.

### pi (`badlogic/pi-mono`, TypeScript, OSS)

- **Log format.** Inferred from `src/core/session-manager.ts`: session is a sequence of `SessionEntry` items, with `CompactionEntry` as a discriminated variant (carries `details: CompactionDetails { readFiles, modifiedFiles }`). Persistence path / wire format wasn't pulled in detail but is JSON-shaped — types live in `pi-agent-core`.
- The agent loop itself (`packages/agent/src/agent-loop.ts`) is pure: takes `prompts: AgentMessage[]`, an `AgentContext`, and a `streamFn`, emits `AgentEvent`s. State management lives one layer up in `coding-agent`.

- **Resume.** Pi has session entries explicitly (`SessionEntry`, `buildSessionContext`); pi's CLI exposes session save/load. The exact resume CLI flag wasn't located in `cli.ts` (file is small — only ~870 bytes — so the bulk of session handling lives in `agent-session.ts` and `session-manager.ts`).

- **Compaction trigger.** Threshold + `/compact`-style flow. Tests in `test/compaction.test.ts`, `test/agent-session-compaction.test.ts`, `test/compaction-summary-reasoning.test.ts`, `test/compaction-extensions.test.ts` confirm: extensible compaction (orgs ship custom compaction via `examples/extensions/custom-compaction.ts`). Built-in includes `branch-summarization.ts` for forking — sub-agents get their own summary.

- **Compaction algorithm** (`src/core/compaction/compaction.ts`):
  - Pure functions; session-manager owns I/O.
  - Calls `completeSimple` with `SUMMARIZATION_SYSTEM_PROMPT` (lives in `core/utils.ts`).
  - Tracks file operations across compactions: walks back through prior `CompactionEntry.details.{readFiles, modifiedFiles}` *plus* extracts file ops from current messages (`extractFileOpsFromMessage`) — explicit, structural file tracking.
  - `serializeConversation` formats history into the summarizer prompt.
  - Exposes `createCompactionSummaryMessage` and `createBranchSummaryMessage` — branch summary is for sub-agent handoff.
  - Extensible: extensions can override compaction (the runner is `core/extensions/runner.ts`).

- **Compaction output shape.** New message of type `compactionSummary` injected; session-manager replaces history with `[summary, recent_tail]`. **`readFiles` and `modifiedFiles` lists are preserved structurally on the `CompactionEntry`** — this is the unique pi feature: the next compaction can read prior file lists rather than re-derive from text. Roughly Codex's `Compacted` marker but with structured file payload.

### Zed (`zed-industries/zed`, Rust, OSS)

- **Log format.** **SQLite**, not JSONL. `crates/agent/src/db.rs` uses `sqlez` (Zed's SQLite wrapper). `DbThread { title, messages: Vec<DbMessage>, updated_at, model: Option<DbLanguageModel>, profile: Option<AgentProfileId>, cumulative_token_usage, request_token_usage, detailed_summary, initial_project_snapshot, draft_prompt, subagent_context, speed, thinking_enabled, thinking_effort, ... }`. `DbThreadMetadata { id: acp::SessionId, parent_session_id, title, updated_at, created_at, folder_paths: PathList }`. Thread sharing via `SharedThread { title, messages, updated_at, model, version: "1.0.0" }`.
- Per-thread record, threads grouped by `folder_paths` (workspace).
- `legacy_thread::DetailedSummaryState` retained as `DbSummary` — i.e. a previous on-disk format is still loaded.

- **Resume.** First-class. `acp::SessionId` is the handle. `parent_session_id` records forks. Threads always loaded from SQLite at startup; the agent panel UI lists/groups them.

- **Compaction trigger.** No fixed threshold visible in `crates/agent/src/thread.rs`. Two prompts in `agent_settings`: `SUMMARIZE_THREAD_PROMPT` and `SUMMARIZE_THREAD_DETAILED_PROMPT` (the prompt strings live in `agent_settings`, not vendored in `agent`). User-triggered: a "summarize" action calls into `Thread::summarize_*`. Auto-compaction does not appear to be implemented in this crate as of HEAD.

- **Compaction algorithm.** `summarization_model: Option<Arc<dyn LanguageModel>>` is a separate model handle from the chat model — Zed lets you pick a cheaper model for summaries and **propagates it to subagents** (test `test_set_summarization_model_propagates_to_subagents`, line 4364). Two prompt variants: short (`SUMMARIZE_THREAD_PROMPT`, line 2677) for titles/recap, detailed (`SUMMARIZE_THREAD_DETAILED_PROMPT`, line 2618) for full handoff. Output stored as `DbThread.detailed_summary: Option<SharedString>`.

- **Compaction output shape.** Summary is *additive* metadata on the thread, not a replacement of the message log. Truncation (`thread.rs:1662 truncate(message_id, ..)`) is a separate operation for explicit user-driven message removal — not summarization.

### Warp (closed)

Source not available; from the warp.dev block-model blog and the Codex unrolling post (which discusses Codex but is referenced for general patterns).

- **Log format.** Not publicly documented at this level of detail. Warp's "Block" abstraction stores command + output as a structured record; the agent layer is built on top of a typed action enum (referenced in hivecore ADR-018: `AIAgentActionType` / `AIAgentActionResultType`) where every agent action and its result is a typed variant carrying risk metadata (`is_read_only`, `is_risky`, `wait_until_completion`, `rationale`, `citations`).
- **Resume.** Warp Drive provides cross-device session-ish persistence; specifics of agent thread resume are not in the blog.
- **Compaction trigger.** The blog states context-window management is "the agent's responsibility" but does not commit to a numeric threshold publicly.
- **Compaction algorithm.** Not described.
- **Compaction output shape.** Inferred to follow industry pattern (summary message + recent tail) but not confirmed.

## Patterns

- **Two writes, one read.** All five OSS tools split (a) the durable on-disk log from (b) the in-memory model payload. Codex/jcode/Claude/pi: append-only JSONL. Zed: SQLite. Compaction never deletes the on-disk log; it only changes what gets sent to the model on the *next* request. jcode and Codex make this explicit (`compacted_count`, `Compacted` marker); Zed treats summary as separate metadata; Claude writes a `compact_boundary` line.

- **Summary message replaces middle, user turns kept verbatim.** Codex's `build_compacted_history(Vec::new(), &user_messages, &summary_text)` is the canonical recipe: drop assistant + tool calls; keep user messages; append synthesized assistant summary. pi does the same shape via `createCompactionSummaryMessage`. jcode uses `compacted_count` + a single summary message — same end state.

- **Threshold values cluster at 80% / 95%.** jcode: 80% background, 95% emergency hard-drop. Claude: 95% default, env-overrideable. Codex: provider-configured `auto_compact_token_limit` (no fixed default in the file). Common emergency fallback: drop oldest items without summary rather than fail.

- **Resume by id or by name; latest-by-cwd is the default UX.** Claude `-c` / `--continue` and `--resume <name|id>` pair is the gold standard. Codex slash `Resume`/`Fork`/`New` mirrors it. jcode supports cross-harness resume by reading other tools' on-disk formats. Zed exposes `acp::SessionId` directly. All tools name sessions as a first-class user-visible string.

- **Markers, not full state, drive replay.** Codex's `Compacted`, `TurnContext`, `SessionMeta` markers in the JSONL let `rollout_reconstruction.rs` rebuild context by scanning newest-to-oldest, finding the most recent surviving checkpoint, then forward-replaying the tail. jcode's `metadata_requires_snapshot` does the same — full snapshots only when meta diverges, otherwise append. Cheap append, expensive checkpoint.

- **Sub-agents complicate but don't break the model.** Claude isolates subagent transcripts in their own `agent-{id}.jsonl` files; main-thread compaction never touches them. Zed propagates the summarization model handle to subagents. pi has a dedicated `branch-summarization.ts` for fork-handoff. Hivecore Layer-1 `SpawnAgentTool` will need the same isolation discipline.

- **Pi's structural file-op tracking is the only departure from "summary-as-text".** `CompactionEntry.details.{readFiles, modifiedFiles}` is data, not prose, and survives compaction. Lets next compaction inherit prior file context without re-deriving from text. Worth copying — closer to the KG ethos than blob-summary.

## Recommendation for hivecore

Concrete proposals for v0.1 / early v0.2.

- **Session log = JSONL, one file per session, in `hivecore-persistence`.** Path `<sessions_dir>/<session-id>.jsonl` where `<session-id>` is `<RFC3339>-<uuid>` (Codex shape). Every line is `RolloutLine { ts, session_id, turn_id, kind, payload }` with `kind` ∈ `{ session_meta, turn_context, response_item, event_msg, compacted, audit }`. Always-persist set: `session_meta`, `turn_context`, `compacted`, plus the seven-plane `audit` event class from ADR-019. This unifies the existing ADR-019 `audit JSONL` with the session rollout — one file, one source of truth, audit-plane is just one event kind.

- **Resume CLI shape.** `hivecore-coder --session <id|name>` (explicit), `hivecore-coder --continue` (latest-by-cwd, no arg), `hivecore-coder --resume` (interactive picker once we have a TUI). Sessions are nameable: `--name <handle>` at start; `/rename <handle>` once running. Picker groups forks under root by `parent_session_id` (Zed/Claude pattern). Resume calls `Session::reconstruct_from_rollout` which scans newest-to-oldest, finds the most recent `compacted` marker, replays the tail forward (Codex `rollout_reconstruction.rs` is the spec).

- **Compaction = `LifecycleHook` plus a `ContextTransform`.** Trigger logic is a `LifecycleHook` firing at `pre_model_request`: it owns the threshold check (default 80% / 95%, both configurable per-agent). Output transformation is a `ContextTransform::transform_outgoing` that the loop calls before every dispatch — receives full history, returns the model-bound slice. v0.1 ship the threshold lifecycle hook + a built-in `SummarizingContextTransform` that mirrors Codex's `build_compacted_history(Vec::new(), &user_messages, &summary_text)` algorithm. v0.2 add jcode-style `Reactive | Proactive | Semantic` modes as alternative hook impls.

- **Compaction output = replace-middle + structural file payload (pi's pattern).** Persist a `Compacted { summary_text, covers_up_to_turn, original_turn_count, read_files, modified_files, tool_calls_summarized }` marker line. In-memory next request = `[initial_context?] + user_messages + Summary(assistant_role)`. The structural `read_files / modified_files` arrays let downstream KG ingestion not lose what files the prior turn touched (closer to UOK Audit-plane than free-form summary). Keep the summary system prompt embeddable / overridable per-agent in the agent TOML — orgs that have stricter handoff requirements (e.g., motadata PMG) need to author their own.

- **Always-keep emergency drop path.** When compaction itself hits `ContextWindowExceeded` (Codex's case) or context is already >95% (jcode's `CRITICAL_THRESHOLD`), drop oldest history items without a summary. This is the "preserve cache prefix, lose oldest tail" rule from `compact.rs`. Surface the drop count as an `event_msg` so it lands in the audit log — never silently lose state.
