use hivecore_runtime_core::{AgentEvent, SessionId, ToolCallId, TurnId};
use tempfile::TempDir;

use super::*;
use hivecore_runtime_core::EventSink;

async fn read_jsonl(path: &std::path::Path) -> Vec<serde_json::Value> {
    let text = tokio::fs::read_to_string(path).await.unwrap();
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn records_orchestrator_and_tool_events() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let trace = SessionId::new();
    let writer = AuditWriter::create(&path, trace, TenantId::single_tenant())
        .await
        .unwrap();

    writer
        .emit(AgentEvent::AgentStart {
            session_id: trace,
            at: chrono::Utc::now(),
        })
        .await;
    let turn = TurnId::new();
    writer
        .emit(AgentEvent::TurnStart {
            turn_id: turn,
            at: chrono::Utc::now(),
        })
        .await;
    writer
        .emit(AgentEvent::ToolExecStart {
            tool_call_id: ToolCallId("c1".into()),
            name: "bash".into(),
            input: serde_json::json!({"command":"ls"}),
        })
        .await;
    writer
        .emit(AgentEvent::ToolExecEnd {
            tool_call_id: ToolCallId("c1".into()),
            is_error: false,
            result: serde_json::Value::Null,
        })
        .await;

    let lines = read_jsonl(&path).await;
    assert_eq!(lines.len(), 4);
    let classes: Vec<&str> = lines.iter().map(|l| l["class"].as_str().unwrap()).collect();
    assert_eq!(
        classes,
        vec!["orchestrator", "orchestrator", "tool", "tool"]
    );
    let kinds: Vec<&str> = lines
        .iter()
        .map(|l| l["payload"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "agent_start",
            "turn_start",
            "tool_exec_start",
            "tool_exec_end"
        ]
    );
}

#[tokio::test]
async fn ignores_streaming_chunks() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let trace = SessionId::new();
    let writer = AuditWriter::create(&path, trace, TenantId::single_tenant())
        .await
        .unwrap();

    writer
        .emit(AgentEvent::MessageDelta {
            message_id: hivecore_runtime_core::MessageId("m".into()),
            delta: hivecore_runtime_core::ContentBlock::Text { text: "x".into() },
        })
        .await;

    let lines = read_jsonl(&path).await;
    assert!(lines.is_empty());
}

#[tokio::test]
async fn tool_end_carries_caused_by_for_paired_start() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let trace = SessionId::new();
    let writer = AuditWriter::create(&path, trace, TenantId::single_tenant())
        .await
        .unwrap();

    writer
        .emit(AgentEvent::ToolExecStart {
            tool_call_id: ToolCallId("c1".into()),
            name: "bash".into(),
            input: serde_json::json!({}),
        })
        .await;
    writer
        .emit(AgentEvent::ToolExecEnd {
            tool_call_id: ToolCallId("c1".into()),
            is_error: false,
            result: serde_json::Value::Null,
        })
        .await;

    let lines = read_jsonl(&path).await;
    let start_id = lines[0]["event_id"].clone();
    let cause = lines[1]["payload"]["caused_by"].clone();
    assert_eq!(start_id, cause);
}
