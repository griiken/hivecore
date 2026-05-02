# crates/hivecore-persistence/AGENTS.md

**Layer 3 — JSONL persistence.** Two `EventSink` impls that drop in alongside the loop.

## What lives here

| Module | Purpose |
|---|---|
| `sessions::SessionWriter` | Pi-style append-only conversation log. One file per `SessionId`. |
| `sessions::SessionReader` | Parses + replays a session log into `Vec<AgentMessage>`. |
| `audit::AuditWriter` | ADR-019 audit-plane writer — `event_id` / `trace_id` / `turn_id` / `caused_by` / `timestamp` / `tenant_id` / `payload`. |
| `audit::AuditClass` | Typed event class enum (`Orchestrator`, `Tool`, `Model`, `Git`, `Test`, `Policy`, `Cost`, `Kg`). |

## Wire formats

**Sessions JSONL** (one entry per line):
```jsonc
{"kind":"header","session_id":"…","model":"gpt-5.4-nano","format_version":1, …}
{"kind":"message","seq":0,"recorded_at":"…","message":{"role":"user", …}}
{"kind":"event","seq":1,"recorded_at":"…","event":{"event":"turn_start", …}}
```

**Audit JSONL**:
```jsonc
{"event_id":"…","trace_id":"<session>","turn_id":"<turn>",
 "tenant_id":"default","class":"tool",
 "payload":{"kind":"tool_exec_start","tool_call_id":"call_x","name":"edit_file","input":{…}}}
```

## Translation rules (audit)

- `AgentEvent::AgentStart/End/TurnStart/End` → class `Orchestrator`.
- `AgentEvent::ToolExecStart/End` → class `Tool`. `ToolExecEnd.caused_by` links back to the `ToolExecStart.event_id` via the per-tool-call pending map.
- Streaming chunks (`MessageStart/Delta/End`, `ToolExecUpdate`, `MessageCommitted`, `Custom`) — *not* audited; they live in the session log only.

## Test commands

```
cargo test -p hivecore-persistence
OPENAI_API_KEY=... cargo run --example persisted_agent
```

## v0.2 backlog

- **ADR-026 Phase A — session resume infrastructure.**
  - Tenant-prefixed paths: `<sessions_dir>/<tenant_id>/<session_id>.jsonl` (default tenant for v0.1).
  - **`session_index.jsonl`** — append-only Codex-pattern index mapping `(id ↔ name ↔ updated_at ↔ cwd)`. Scan-from-EOF on lookup. Most recent line wins.
  - **Advisory lock file** `<session_id>.jsonl.lock` written on open / removed on close. Second process refuses with "session in use, --fork" hint.
  - `find_latest_by_cwd(cwd)` for `--continue`. `find_by_id_or_name(handle)` for `--session`.
  - `SessionReader::messages()` already exists — no change needed for read path.
- **ADR-026 — `compaction_marker` is a `Custom` message kind.** `SessionEntry` schema needs no extension — markers ride the existing message channel. Document the kind name + payload schema in `SessionEntry` doc-comments.
- **Sub-agent path layout (ADR-026):** `<sessions_dir>/<tenant_id>/<parent_id>/subagents/<child_id>.jsonl`.
- SQLite / Postgres projection layer (ADR-019 mentions both).
