use super::*;

#[test]
fn user_message_roundtrips_through_json() {
    let original = AgentMessage::User {
        id: MessageId("m1".into()),
        content: vec![ContentBlock::Text {
            text: "hello".into(),
        }],
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let back: AgentMessage = serde_json::from_str(&json).expect("deserialize");

    match back {
        AgentMessage::User { id, content } => {
            assert_eq!(id.0, "m1");
            assert_eq!(content.len(), 1);
        }
        other => panic!("expected User, got {other:?}"),
    }
}

#[test]
fn custom_message_visibility_default_for_other_variants() {
    let user = AgentMessage::User {
        id: MessageId("m".into()),
        content: vec![],
    };
    assert!(user.visible_to_model());

    let hidden = AgentMessage::Custom {
        id: MessageId("c".into()),
        kind: "bookmark".into(),
        payload: serde_json::json!({}),
        visible_to_model: false,
    };
    assert!(!hidden.visible_to_model());
}

#[test]
fn id_accessor_returns_each_variant_id() {
    let m = AgentMessage::ToolResult {
        id: MessageId("r1".into()),
        tool_call_id: ToolCallId("t1".into()),
        content: vec![],
        is_error: false,
    };
    assert_eq!(m.id().0, "r1");
}
