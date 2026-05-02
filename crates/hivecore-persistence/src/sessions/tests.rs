use hivecore_runtime_core::{
    AgentEvent, AgentMessage, ContentBlock, MessageId, SessionId, StopReason, TurnId,
};
use tempfile::TempDir;

use super::*;
use hivecore_runtime_core::EventSink;

fn user(text: &str) -> AgentMessage {
    AgentMessage::User {
        id: MessageId(text.into()),
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn assistant(text: &str) -> AgentMessage {
    AgentMessage::Assistant {
        id: MessageId(format!("a-{text}")),
        content: vec![ContentBlock::Text { text: text.into() }],
        stop_reason: Some(StopReason::EndTurn),
    }
}

#[tokio::test]
async fn writes_header_messages_events_and_replays() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join(format!("{}.jsonl", session.0));
    let header = SessionHeader::new(session, "gpt-5.4-nano", "system");

    let writer = SessionWriter::create(&path, header.clone()).await.unwrap();

    writer
        .emit(AgentEvent::MessageCommitted {
            message: user("hi"),
        })
        .await;
    writer
        .emit(AgentEvent::TurnStart {
            turn_id: TurnId::new(),
            at: chrono::Utc::now(),
        })
        .await;
    writer
        .emit(AgentEvent::MessageCommitted {
            message: assistant("hello back"),
        })
        .await;

    let r = SessionReader::open(&path).await.unwrap();
    assert_eq!(r.header.session_id.0, session.0);
    let msgs = r.messages();
    assert_eq!(msgs.len(), 2);
    assert!(matches!(&msgs[0], AgentMessage::User { .. }));
    assert!(matches!(&msgs[1], AgentMessage::Assistant { .. }));
    assert_eq!(r.event_count(), 1); // TurnStart only
}

#[tokio::test]
async fn rejects_file_missing_header() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.jsonl");
    tokio::fs::write(&path, "{\"kind\":\"event\"}\n")
        .await
        .unwrap();
    let err = SessionReader::open(&path).await.unwrap_err();
    assert!(format!("{err}").contains("missing header") || format!("{err}").contains("decode"));
}

#[tokio::test]
async fn seq_monotonic_across_emits() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    for i in 0..5 {
        writer
            .emit(AgentEvent::MessageCommitted {
                message: user(&format!("m{i}")),
            })
            .await;
    }
    let r = SessionReader::open(&path).await.unwrap();
    let mut last = None;
    for e in &r.entries {
        if let SessionEntry::Message { seq, .. } = e {
            if let Some(prev) = last {
                assert!(*seq > prev);
            }
            last = Some(*seq);
        }
    }
}
