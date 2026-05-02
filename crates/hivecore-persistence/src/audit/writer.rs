//! Audit-plane writer. Translates `AgentEvent` into `AuditEvent`s and
//! appends them as JSONL. `caused_by` linkage is best-effort: a single
//! pending event_id per `tool_call_id` pairs `ToolExecStart` with its
//! matching `ToolExecEnd`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{AgentEvent, SessionId, ToolCallId};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::schema::{AuditClass, AuditEvent, EventId, TenantId};
use crate::error::Result;
use hivecore_runtime_core::EventSink;

#[derive(Clone)]
pub struct AuditWriter {
    inner: Arc<Mutex<Inner>>,
    trace_id: SessionId,
    tenant_id: TenantId,
    path: Arc<PathBuf>,
}

impl std::fmt::Debug for AuditWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditWriter")
            .field("path", &self.path)
            .field("trace_id", &self.trace_id)
            .finish()
    }
}

struct Inner {
    file: tokio::fs::File,
    pending_tool: HashMap<ToolCallId, EventId>,
}

impl AuditWriter {
    pub async fn create(
        path: impl AsRef<Path>,
        trace_id: SessionId,
        tenant_id: TenantId,
    ) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                file,
                pending_tool: HashMap::new(),
            })),
            trace_id,
            tenant_id,
            path: Arc::new(path),
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub async fn write_raw(&self, event: AuditEvent) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let mut line = serde_json::to_string(&event)?;
        line.push('\n');
        inner.file.write_all(line.as_bytes()).await?;
        inner.file.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl EventSink for AuditWriter {
    async fn emit(&self, event: AgentEvent) {
        let translated = match event {
            AgentEvent::AgentStart { session_id, at } => Some((
                AuditClass::Orchestrator,
                serde_json::json!({"kind":"agent_start","session_id": session_id.0, "at": at}),
                None,
                None,
            )),
            AgentEvent::AgentEnd {
                session_id,
                at,
                reason,
            } => Some((
                AuditClass::Orchestrator,
                serde_json::json!({"kind":"agent_end","session_id": session_id.0, "at": at, "reason": reason}),
                None,
                None,
            )),
            AgentEvent::TurnStart { turn_id, at } => Some((
                AuditClass::Orchestrator,
                serde_json::json!({"kind":"turn_start","at": at}),
                Some(turn_id),
                None,
            )),
            AgentEvent::TurnEnd {
                turn_id,
                at,
                stop_reason,
            } => Some((
                AuditClass::Orchestrator,
                serde_json::json!({"kind":"turn_end","at": at,"stop_reason": stop_reason}),
                Some(turn_id),
                None,
            )),
            AgentEvent::ToolExecStart {
                tool_call_id,
                name,
                input,
            } => {
                let event_id = EventId::new();
                self.inner
                    .lock()
                    .await
                    .pending_tool
                    .insert(tool_call_id.clone(), event_id);
                Some((
                    AuditClass::Tool,
                    serde_json::json!({"kind":"tool_exec_start","tool_call_id": tool_call_id.0,"name": name,"input": input}),
                    None,
                    Some(event_id),
                ))
            }
            AgentEvent::ToolExecEnd {
                tool_call_id,
                is_error,
                result,
            } => {
                let parent = self.inner.lock().await.pending_tool.remove(&tool_call_id);
                let mut payload = serde_json::json!({"kind":"tool_exec_end","tool_call_id": tool_call_id.0,"is_error": is_error,"result": result});
                if let Some(parent_id) = parent {
                    payload["caused_by"] = serde_json::json!(parent_id);
                }
                Some((AuditClass::Tool, payload, None, None))
            }
            AgentEvent::MessageStart { .. }
            | AgentEvent::MessageDelta { .. }
            | AgentEvent::MessageEnd { .. }
            | AgentEvent::ToolExecUpdate { .. }
            | AgentEvent::MessageCommitted { .. }
            | AgentEvent::Custom { .. } => {
                // Streaming chunks and message commits stay in the session
                // log; the audit plane records orchestration, tool, and
                // policy-class events only.
                None
            }
        };

        let Some((class, payload, turn, override_id)) = translated else {
            return;
        };

        let mut audit = AuditEvent::new(self.trace_id, self.tenant_id.clone(), class, payload);
        if let Some(t) = turn {
            audit = audit.with_turn(t);
        }
        if let Some(id) = override_id {
            audit.event_id = id;
        }

        if let Err(e) = self.write_raw(audit).await {
            tracing::warn!(error = %e, "audit writer dropped event");
        }
    }
}
