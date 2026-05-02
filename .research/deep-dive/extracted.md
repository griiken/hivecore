# DEEP DIVE — session resume + compaction

## CODEX (`openai/codex`, Rust)

### Constants

- `COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000`

### build_compacted_history
```rust
fn build_compacted_history(
    initial_context: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
) -> Vec<ResponseItem> {
    build_compacted_history_with_limit(
        initial_context,
        user_messages,
        summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS,
    )
}
```

### run_inline_auto_compact_task
```rust
async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text {
        text: prompt,
        // Compaction prompt is synthesized; no UI element ranges to preserve.
        text_elements: Vec::new(),
    }];

    run_compact_task_inner(
        sess,
        turn_context,
        input,
        initial_context_injection,
        CompactionTrigger::Auto,
        reason,
        phase,
    )
    .await?;
    Ok(())
}
```

### run_compact_task
```rust
async fn run_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) -> CodexResult<()> {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_context.sub_id.clone(),
        started_at: turn_context.turn_timing_state.started_at_unix_secs().await,
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;
    run_compact_task_inner(
        sess.clone(),
        turn_context,
        input,
        InitialContextInjection::DoNotInject,
        CompactionTrigger::Manual,
        CompactionReason::UserRequested,
        CompactionPhase::StandaloneTurn,
    )
    .await
}
```

### include_str refs in compact.rs: ['../templates/compact/prompt.md', '../templates/compact/summary_prefix.md']

### Compact prompt body (`templates/compact/prompt.md`)
```


You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task.

Include:
- Current progress and key decisions made
- Important context, constraints, or user preferences
- What remains to be done (clear next steps)
- Any critical data, examples, or references needed to continue

Be concise, structured, and focused on helping the next LLM seamlessly continue the work.

## branch summarization sub agent fork handoff

### Pi branch-summarization.ts > Progress
## Progress

### Pi branch-summarization.ts > Goal
## Goal
[What was the user trying to accomplish in this branch?]

### Pi branch-summarization.ts > Progress > Done
### Done
- [x] [Completed tasks/changes]

## DbThread detailed_summary cumulative_token_usage Zed

### Zed agent db.rs (1)

```

### Summary-prefix preamble (`templates/compact/summary_prefix.md`)
```
Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:
```

### ContextWindowExceeded mentions in compact.rs
```
            Err(e @ CodexErr::ContextWindowExceeded) => {
```

### Drop-oldest fallback
```
                    // Trim from the beginning to preserve cache (prefix-based) and keep recent messages intact.
                    history.remove_first_item();
```

## Codex rollout/policy.rs (what gets persisted)

### should_persist_event_msg
```rust
pub fn should_persist_event_msg(ev: &EventMsg, mode: EventPersistenceMode) -> bool {
    match mode {
        EventPersistenceMode::Limited => should_persist_event_msg_limited(ev),
        EventPersistenceMode::Extended => should_persist_event_msg_extended(ev),
    }
}
```

## Codex rollout_reconstruction.rs

## Codex rollout/session_index.rs

### SessionIndex struct
```rust
pub struct SessionIndexEntry {
    pub id: ThreadId,
    pub thread_name: String,
    pub updated_at: String,
}
```

### top of session_index.rs
```rust
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMetaLine;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;

const SESSION_INDEX_FILE: &str = "session_index.jsonl";
const READ_CHUNK_SIZE: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIndexEntry {
    pub id: ThreadId,
    pub thread_name: String,
    pub updated_at: String,
}

/// Append a thread name update to the session index.
/// The index is append-only; the most recent entry wins when resolving names or ids.
pub async fn append_thread_name(
    codex_home: &Path,
    thread_id: ThreadId,
    name: &str,
) -> std::io::Result<()> {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    let updated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let entry = SessionIndexEntry {
        id: thread_id,
        thread_name: name.to_string(),
        updated_at,
    };
    append_session_index_entry(codex_home, &entry).await
}

/// Append a raw session index entry to `session_index.jsonl`.
/// The file is append-only; consumers scan from the end to find the newest match.
pub async fn append_session_index_entry(
    codex_home: &Path,
    entry: &SessionIndexEntry,
) -> std::io::Result<()> {
    let path = session_index_path(codex_home);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    let mut line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    line.push('\n');
    file.write_all(line.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Find the latest thread name for a thread id, if any.
pub async fn find_thread_name_by_id(
    codex_home: &Path,
    thread_id: &ThreadId,
) -> std::io::Result<Option<String>> {
    let path = session_index_path(codex_home);
    if !path.exists() {
        return Ok(None);
    }
    let id = *thread_id;
    let entry = tokio::task::spawn_blocking(move || scan_index_from_end_by_id(&path, &id))
        .await
        .map_err(std::io::Error::other)??;
    Ok(entry.map(|entry| entry.thread_name))
}

/// Find the latest thread names for a batch of thread ids.
pub async fn find_thread_names_by_ids(
    codex_home: &Path,
    thread_ids: &HashSet<ThreadId>,
) -> std::io::Result<HashMap<ThreadId, String>> {
    let path = session_index_path(codex_home);
    if thread_ids.is_empty() || !path.exists() {
        return Ok(HashMap::new());
    }

    let file = tokio::fs::File::open(&path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut names = HashMap::with_capacity(thread_ids.len());

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
            continue;
        };
        let name = entry.thread_name.trim();
        if !name.is_empty() && thread_ids.contains(&entry.id) {
            names.insert(entry.id, name.to_string());
        }
    }

    Ok(names)
}

/// Locate a recorded thread rollout and read its s
```

## PI (`badlogic/pi-mono`, TS)

## pi compaction.ts

### SUMMARIZATION_PROMPT
```
The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work.

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

### exports
```
CompactionDetails
CompactionResult
CompactionSettings
DEFAULT_COMPACTION_SETTINGS
calculateContextTokens
getLastAssistantUsage
ContextUsageEstimate
estimateContextTokens
shouldCompact
estimateTokens
findTurnStartIndex
CutPointResult
findCutPoint
generateSummary
CompactionPreparation
prepareCompaction
compact
```

- `compact` at offset 22733: `export async function compact(
	preparation: CompactionPreparation,
	model: Model<any>,
	apiKey: string,
	headers?: Record<string, string>,
	customInstructions?: string,
	signal?: AbortSignal,
	thinki...`

## pi branch-summarization.ts

### exports
```
BranchSummaryResult
BranchSummaryDetails
BranchPreparation
CollectEntriesResult
GenerateBranchSummaryOptions
```

## pi session-manager.ts

### exports
```
CURRENT_SESSION_VERSION
SessionHeader
NewSessionOptions
SessionEntryBase
SessionMessageEntry
ThinkingLevelChangeEntry
ModelChangeEntry
CompactionEntry
BranchSummaryEntry
CustomEntry
LabelEntry
SessionInfoEntry
CustomMessageEntry
SessionEntry
FileEntry
SessionTreeNode
SessionContext
SessionInfo
ReadonlySessionManager
migrateSessionEntries
parseSessionEntries
getLatestCompactionEntry
buildSessionContext
getDefaultSessionDir
loadEntriesFromFile
findMostRecentSession
SessionListProgress
SessionManager
```

### `CompactionEntry` mentions (7)
```
export interface CompactionEntry<T = unknown> extends SessionEntryBase {
	| CompactionEntry
			const comp = entry as CompactionEntry & { firstKeptEntryIndex?: number };
export function getLatestCompactionEntry(entries: SessionEntry[]): CompactionEntry | null {
			return entries[i] as CompactionEntry;
	let compaction: CompactionEntry | null = null;
```
### `sessionId` mentions (8)
```
	private sessionId: string = "";
			this.sessionId = header?.id ?? createSessionId();
		this.sessionId = options?.id ?? createSessionId();
			id: this.sessionId,
			this.sessionFile = join(this.getSessionDir(), `${fileTimestamp}_${this.sessionId}.jsonl`);
		return this.sessionId;
```
### `resume` mentions (1)
```
	/** Switch to a different session file (used for resume and branching) */
```
## ZED (`zed-industries/zed`, Rust)

## zed thread.rs (compaction sections)

### prompt match
```
SUMMARIZE_THREAD_PROMPT,
};
use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Utc};
use client::UserStore;
use cloud_api_types::Plan;
use collections::{HashMap, HashSet, IndexMap};
use fs::Fs;
use futures::{
    FutureExt,
    channel::{mpsc, oneshot},
    future::Shared,
    stream::FuturesUnordered,
};
use futures::{StreamExt, stream};
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, EventEmitter, SharedString, Task, WeakEntity,
};
use heck::ToSnakeCase as _;
use language_model::{
    CompletionIntent, LanguageModel, LanguageModelCompletionError, LanguageModelCompletionEvent,
    LanguageModelId, LanguageModelImage, LanguageModelProviderId, LanguageModelRegistry,
    LanguageModelRequest, LanguageModelRequestMessage, LanguageModelRequestTool,
    LanguageModelToolResult, LanguageModelToolResultContent, LanguageModelToolSchemaFormat,
    LanguageModelToolUse, LanguageModelToolUseId, Role, SelectedModel, Speed, StopReason,
    TokenUsage, ZED_CLOUD_PROVIDER_ID,
};
use project::Project;
use prompt_store::ProjectContext;
use schemars::{JsonSchema, Schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use settings::{
    LanguageModelSelection, Settings, SettingsStore, ToolPermissionMode, update_settings_file,
};
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    ops::RangeInclusive,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use std::{fmt::Write, path::PathBuf};
use util::{Res
```

### prompt match
```
SUMMARIZE_THREAD_DETAILED_PROMPT, SUMMARIZE_THREAD_PROMPT,
};
use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Utc};
use client::UserStore;
use cloud_api_types::Plan;
use collections::{HashMap, HashSet, IndexMap};
use fs::Fs;
use futures::{
    FutureExt,
    channel::{mpsc, oneshot},
    future::Shared,
    stream::FuturesUnordered,
};
use futures::{StreamExt, stream};
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, EventEmitter, SharedString, Task, WeakEntity,
};
use heck::ToSnakeCase as _;
use language_model::{
    CompletionIntent, LanguageModel, LanguageModelCompletionError, LanguageModelCompletionEvent,
    LanguageModelId, LanguageModelImage, LanguageModelProviderId, LanguageModelRegistry,
    LanguageModelRequest, LanguageModelRequestMessage, LanguageModelRequestTool,
    LanguageModelToolResult, LanguageModelToolResultContent, LanguageModelToolSchemaFormat,
    LanguageModelToolUse, LanguageModelToolUseId, Role, SelectedModel, Speed, StopReason,
    TokenUsage, ZED_CLOUD_PROVIDER_ID,
};
use project::Project;
use prompt_store::ProjectContext;
use schemars::{JsonSchema, Schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use settings::{
    LanguageModelSelection, Settings, SettingsStore, ToolPermissionMode, update_settings_file,
};
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    ops::RangeInclusive,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use std::{fmt::Wri
```

### prompt match
```
SUMMARIZE_THREAD_DETAILED_PROMPT, SUMMARIZE_THREAD_PROMPT,
};
use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Utc};
use client::UserStore;
use cloud_api_types::Plan;
use collections::{HashMap, HashSet, IndexMap};
use fs::Fs;
use futures::{
    FutureExt,
    channel::{mpsc, oneshot},
    future::Shared,
    stream::FuturesUnordered,
};
use futures::{StreamExt, stream};
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, EventEmitter, SharedString, Task, WeakEntity,
};
use heck::ToSnakeCase as _;
use language_model::{
    CompletionIntent, LanguageModel, LanguageModelCompletionError, LanguageModelCompletionEvent,
    LanguageModelId, LanguageModelImage, LanguageModelProviderId, LanguageModelRegistry,
    LanguageModelRequest, LanguageModelRequestMessage, LanguageModelRequestTool,
    LanguageModelToolResult, LanguageModelToolResultContent, LanguageModelToolSchemaFormat,
    LanguageModelToolUse, LanguageModelToolUseId, Role, SelectedModel, Speed, StopReason,
    TokenUsage, ZED_CLOUD_PROVIDER_ID,
};
use project::Project;
use prompt_store::ProjectContext;
use schemars::{JsonSchema, Schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use settings::{
    LanguageModelSelection, Settings, SettingsStore, ToolPermissionMode, update_settings_file,
};
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    ops::RangeInclusive,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use std::{fmt::Wri
```

- summarization_model: Option<Arc<dyn LanguageModel>>,
- summarization_model: None,
- self.summarization_model = parent.summarization_model.clone();
- summarization_model: None,
- pub fn summarization_model(&self) -> Option<&Arc<dyn LanguageModel>> {
- self.summarization_model.as_ref()
- self.summarization_model = model.clone();
- let Some(model) = self.summarization_model.clone() else {
- log::error!("No summarization model available");
- let Some(model) = self.summarization_model.clone() else {
- self.summarization_model.as_ref().map(|model| model.name())
- let subagent_summary_id = subagent.read(cx).summarization_model().unwrap().id();
- "Subagent summarization model should match parent after set_summarization_model"
## zed db.rs DbThread

```rust
pub struct DbThread {
    pub title: SharedString,
    pub messages: Vec<DbMessage>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub detailed_summary: Option<SharedString>,
    #[serde(default)]
    pub initial_project_snapshot: Option<Arc<crate::ProjectSnapshot>>,
    #[serde(default)]
    pub cumulative_token_usage: language_model::TokenUsage,
    #[serde(default)]
    pub request_token_usage: HashMap<acp_thread::UserMessageId, language_model::TokenUsage>,
    #[serde(default)]
    pub model: Option<DbLanguageModel>,
    #[serde(default)]
    pub profile: Option<AgentProfileId>,
    #[se…

…Thread {
    pub title: SharedString,
    pub messages: Vec<DbMessage>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub model: Option<DbLanguageModel>,
    pub version: String,
}
```

```rust
pub struct DbThreadMetadata {
    pub id: acp::SessionId,
    pub parent_session_id…

…eta.id.clone(),
            work_dirs: Some(meta.folder_paths.clone()),
            title: Some(meta.title.clone()),
            updated_at: Some(meta.updated_at),
            created_at: meta.created_at,
            meta: None,
        }
```

## JCODE (`1jehuang/jcode`, Rust)

## jcode compaction.rs

### Constants

- `DEFAULT_TOKEN_BUDGET: usize = 200_000`
- `COMPACTION_THRESHOLD: f32 = 0.80`
- `CRITICAL_THRESHOLD: f32 = 0.95`
- `MANUAL_COMPACT_MIN_THRESHOLD: f32 = 0.10`
- `RECENT_TURNS_TO_KEEP: usize = 10`
- `MIN_TURNS_TO_KEEP: usize = 2`
- `EMERGENCY_TOOL_RESULT_MAX_CHARS: usize = 4000`
- `CHARS_PER_TOKEN: usize = 4`
- `SYSTEM_OVERHEAD_TOKENS: usize = 18_000`
- `TOKEN_HISTORY_WINDOW: usize = 20`
- `EMBED_MAX_CHARS_PER_MSG: usize = 512`
- `EMBEDDING_HISTORY_WINDOW: usize = 10`
- `DEFAULT_TOKEN_BUDGET: usize = 200_000`
- `COMPACTION_THRESHOLD: f32 = 0.80`
- `CRITICAL_THRESHOLD: f32 = 0.95`
- `MANUAL_COMPACT_MIN_THRESHOLD: f32 = 0.10`
- `RECENT_TURNS_TO_KEEP: usize = 10`
- `MIN_TURNS_TO_KEEP: usize = 2`
- `EMERGENCY_TOOL_RESULT_MAX_CHARS: usize = 4000`
- `CHARS_PER_TOKEN: usize = 4`
- `SYSTEM_OVERHEAD_TOKENS: usize = 18_000`
- `TOKEN_HISTORY_WINDOW: usize = 20`
- `EMBED_MAX_CHARS_PER_MSG: usize = 512`
- `EMBEDDING_HISTORY_WINDOW: usize = 10`
