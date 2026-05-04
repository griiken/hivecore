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

// -- ADR-032c: SessionStore trait ----------------------------------------

#[tokio::test]
async fn store_trait_round_trips_via_arc_dyn() {
    use std::sync::Arc;

    use crate::sessions::store::{SessionEntryKind, SessionStore};

    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    writer
        .emit(AgentEvent::MessageCommitted { message: user("a") })
        .await;
    writer
        .emit(AgentEvent::MessageCommitted {
            message: assistant("b"),
        })
        .await;

    let store: Arc<dyn SessionStore> = Arc::new(writer);
    let meta = store.metadata().await.unwrap();
    assert_eq!(meta.session_id, session);
    assert_eq!(meta.format_version, 2);

    let leaf = store.leaf_id().await.unwrap();
    assert!(leaf.is_some());

    let path_msgs = store.path_to_root(None).await.unwrap();
    let msg_count = path_msgs
        .iter()
        .filter(|e| matches!(e, SessionEntry::Message { .. }))
        .count();
    assert_eq!(msg_count, 2);

    let by_kind = store
        .find_entries_of_kind(SessionEntryKind::Message)
        .await
        .unwrap();
    assert_eq!(by_kind.len(), 2);

    let leaf_changes = store
        .find_entries_of_kind(SessionEntryKind::LeafChange)
        .await
        .unwrap();
    assert_eq!(leaf_changes.len(), 2);

    // children_of: the first message has the LeafChange after it as a
    // child; the LeafChange has the second message as a child.
    let first_msg_id = match &path_msgs[0] {
        SessionEntry::Message { id, .. } => id.clone(),
        _ => unreachable!(),
    };
    let kids = store.children_of(&first_msg_id).await.unwrap();
    assert!(
        !kids.is_empty(),
        "first message must have at least one child"
    );

    // get_entry round-trip.
    let fetched = store.get_entry(&first_msg_id).await.unwrap();
    assert!(fetched.is_some());
}

// -- ADR-032b: fork via set_leaf_id --------------------------------------

#[tokio::test]
async fn fork_from_mid_tree_orphans_old_branch() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    // Linear: m1 → m2 → m3.
    for tag in ["m1", "m2", "m3"] {
        writer
            .emit(AgentEvent::MessageCommitted { message: user(tag) })
            .await;
    }

    // Fork: rewind to m1 and append m4 there. m2 and m3 stay on disk
    // but become unreferenced (orphaned) — replay from new leaf
    // returns [m1, m4].
    let snap = SessionReader::open(&path).await.unwrap();
    let m1_id = snap
        .entries
        .iter()
        .find_map(|e| match e {
            SessionEntry::Message {
                id,
                message: AgentMessage::User { content, .. },
                ..
            } if matches!(&content[0], ContentBlock::Text { text } if text == "m1") => {
                Some(id.clone())
            }
            _ => None,
        })
        .expect("m1 exists");
    writer.set_leaf_id(Some(m1_id.clone())).await.unwrap();
    writer
        .emit(AgentEvent::MessageCommitted {
            message: user("m4"),
        })
        .await;

    let r = SessionReader::open(&path).await.unwrap();
    let texts: Vec<String> = r
        .messages()
        .iter()
        .filter_map(|m| match m {
            AgentMessage::User { content, .. } => match &content[0] {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec!["m1", "m4"],
        "fork replay should drop orphaned m2/m3"
    );

    // m2 and m3 are still on disk (append-only invariant).
    let on_disk_msg_count = r
        .entries
        .iter()
        .filter(|e| matches!(e, SessionEntry::Message { .. }))
        .count();
    assert_eq!(on_disk_msg_count, 4, "all 4 messages must remain on disk");
}

#[tokio::test]
async fn set_leaf_id_writes_leaf_change_entry() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    writer
        .emit(AgentEvent::MessageCommitted { message: user("a") })
        .await;
    let leaf_change_id = writer.set_leaf_id(None).await.unwrap();
    assert!(!leaf_change_id.as_str().is_empty());
    let r = SessionReader::open(&path).await.unwrap();
    assert_eq!(r.leaf_id(), None);
}

// -- ADR-032: tree-shaped persistence ------------------------------------

#[tokio::test]
async fn entries_form_parent_chain() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    writer
        .emit(AgentEvent::MessageCommitted { message: user("a") })
        .await;
    writer
        .emit(AgentEvent::MessageCommitted {
            message: assistant("b"),
        })
        .await;

    let r = SessionReader::open(&path).await.unwrap();

    // Two Messages + two LeafChanges.
    let msgs: Vec<_> = r
        .entries
        .iter()
        .filter(|e| matches!(e, SessionEntry::Message { .. }))
        .collect();
    assert_eq!(msgs.len(), 2);
    let leaf_changes: Vec<_> = r
        .entries
        .iter()
        .filter(|e| matches!(e, SessionEntry::LeafChange { .. }))
        .collect();
    assert_eq!(leaf_changes.len(), 2);

    // First Message has parent=None (root); second's parent is first's id.
    let m1 = match msgs[0] {
        SessionEntry::Message { id, parent_id, .. } => (id.clone(), parent_id.clone()),
        _ => unreachable!(),
    };
    let m2 = match msgs[1] {
        SessionEntry::Message { id, parent_id, .. } => (id.clone(), parent_id.clone()),
        _ => unreachable!(),
    };
    assert!(m1.1.is_none(), "first message should have no parent");
    // m2's parent is the leaf at the time of append, which after the first
    // `append_message` is the LeafChange's `to` = m1.id. The first
    // LeafChange itself sits between the two Messages on disk; the cursor
    // walks through Message ids only via parent_id chains:
    // m1 ← LeafChange(parent=m1) ← m2(parent=m1).
    assert_eq!(m2.1.as_ref(), Some(&m1.0), "m2 should chain to m1");
}

#[tokio::test]
async fn leaf_id_tracks_latest_message() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    writer
        .emit(AgentEvent::MessageCommitted { message: user("a") })
        .await;
    writer
        .emit(AgentEvent::MessageCommitted {
            message: assistant("b"),
        })
        .await;

    let r = SessionReader::open(&path).await.unwrap();
    let last_msg_id = r.entries.iter().rev().find_map(|e| match e {
        SessionEntry::Message { id, .. } => Some(id.clone()),
        _ => None,
    });
    assert_eq!(r.leaf_id(), last_msg_id);
}

#[tokio::test]
async fn path_to_root_returns_messages_in_chronological_order() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    for tag in ["a", "b", "c", "d"] {
        writer
            .emit(AgentEvent::MessageCommitted { message: user(tag) })
            .await;
    }
    let r = SessionReader::open(&path).await.unwrap();
    let msgs = r.messages();
    assert_eq!(msgs.len(), 4);
    let texts: Vec<String> = msgs
        .iter()
        .map(|m| match m {
            AgentMessage::User { content, .. } => match &content[0] {
                ContentBlock::Text { text } => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        })
        .collect();
    assert_eq!(texts, vec!["a", "b", "c", "d"]);
}

#[tokio::test]
async fn resume_preserves_cursor_continuity() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer1 = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    writer1
        .emit(AgentEvent::MessageCommitted {
            message: user("first"),
        })
        .await;
    drop(writer1);

    // Resume — read state, reopen with seeded cursor, append another.
    let snap = SessionReader::open(&path).await.unwrap();
    let leaf = snap.leaf_id();
    let ids = snap.issued_ids();
    assert!(leaf.is_some());

    let writer2 = SessionWriter::open_append_with_state(&path, leaf.clone(), ids)
        .await
        .unwrap();
    writer2
        .emit(AgentEvent::MessageCommitted {
            message: assistant("second"),
        })
        .await;

    // Re-read everything; the second message's parent must be the first
    // message's id (cursor was preserved across the resume).
    let r = SessionReader::open(&path).await.unwrap();
    let msg_entries: Vec<_> = r
        .entries
        .iter()
        .filter_map(|e| match e {
            SessionEntry::Message { id, parent_id, .. } => Some((id.clone(), parent_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(msg_entries.len(), 2);
    assert!(msg_entries[0].1.is_none());
    assert_eq!(msg_entries[1].1, Some(msg_entries[0].0.clone()));

    // Replay yields both messages in order.
    let texts: Vec<String> = r
        .messages()
        .iter()
        .filter_map(|m| match m {
            AgentMessage::User { content, .. } | AgentMessage::Assistant { content, .. } => {
                match &content[0] {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["first", "second"]);
}

#[tokio::test]
async fn entry_ids_are_unique() {
    let dir = TempDir::new().unwrap();
    let session = SessionId::new();
    let path = dir.path().join("s.jsonl");
    let writer = SessionWriter::create(&path, SessionHeader::new(session, "m", "s"))
        .await
        .unwrap();
    for i in 0..50 {
        writer
            .emit(AgentEvent::MessageCommitted {
                message: user(&format!("m{i}")),
            })
            .await;
    }
    let r = SessionReader::open(&path).await.unwrap();
    let mut seen: std::collections::HashSet<EntryId> = std::collections::HashSet::new();
    for entry in &r.entries {
        if let Some(id) = entry.id() {
            assert!(seen.insert(id.clone()), "duplicate id `{}`", id.as_str());
        }
    }
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
