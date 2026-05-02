//! `EventSink` that pretty-prints lifecycle events to stderr.
//!
//! Different style than `hivecore-acp-server`'s bridge sink (which marshals
//! to ACP `session/update` notifications). Same trait, different impl —
//! the substrate doesn't care which.

use async_trait::async_trait;
use hivecore_runtime_core::{AgentEvent, EventSink};

#[derive(Debug, Default, Clone)]
pub struct StderrSink;

#[async_trait]
impl EventSink for StderrSink {
    async fn emit(&self, event: AgentEvent) {
        match event {
            AgentEvent::AgentStart { session_id, .. } => {
                eprintln!("[start] session={}", session_id.0);
            }
            AgentEvent::TurnStart { turn_id, .. } => {
                eprintln!("[turn ] {}", turn_id.0);
            }
            AgentEvent::ToolExecStart { name, input, .. } => {
                eprintln!("[tool ] {name} {input}");
            }
            AgentEvent::ToolExecEnd { is_error, .. } => {
                let tag = if is_error { "ERR " } else { "ok  " };
                eprintln!("[done ] {tag}");
            }
            AgentEvent::TurnEnd {
                turn_id,
                stop_reason,
                ..
            } => {
                eprintln!("[stop ] turn={} reason={stop_reason:?}", turn_id.0);
            }
            AgentEvent::AgentEnd { reason, .. } => {
                eprintln!("[end  ] reason={reason:?}");
            }
            _ => {}
        }
    }
}
