use super::*;
use hivecore_runtime_core::{
    AgentMessage, ContentBlock, MessageId, ModelChunk, ModelRequest, ThinkingLevel, ToolCallId,
    ToolDescriptor,
};
use serde_json::json;

fn empty_req(messages: Vec<AgentMessage>) -> ModelRequest {
    ModelRequest {
        model: "gpt-4o-mini".into(),
        system: "be brief".into(),
        messages,
        tools: vec![],
        thinking: ThinkingLevel::Off,
        max_tokens: Some(256),
    }
}

#[test]
fn build_request_prepends_system() {
    let req = empty_req(vec![AgentMessage::User {
        id: MessageId("m".into()),
        content: vec![ContentBlock::Text { text: "hi".into() }],
    }]);
    let wire = build_request(&req, true).unwrap();
    assert!(matches!(wire.messages[0], ChatMessage::System { .. }));
    assert!(matches!(wire.messages[1], ChatMessage::User { .. }));
}

#[test]
fn build_request_skips_invisible_custom() {
    let req = empty_req(vec![AgentMessage::Custom {
        id: MessageId("c".into()),
        kind: "bookmark".into(),
        payload: json!({}),
        visible_to_model: false,
    }]);
    let wire = build_request(&req, false).unwrap();
    assert_eq!(wire.messages.len(), 1); // system only
}

#[test]
fn build_request_emits_tools() {
    let mut req = empty_req(vec![]);
    req.tools.push(ToolDescriptor {
        name: "grep".into(),
        description: "search".into(),
        parameters: json!({"type":"object"}),
    });
    let wire = build_request(&req, true).unwrap();
    let tools = wire.tools.expect("tools present");
    assert_eq!(tools[0].function.name, "grep");
}

#[test]
fn aggregator_emits_message_start_then_text_then_end() {
    let mut agg = StreamAggregator::new();
    let chunks = agg
        .ingest(serde_json::from_str(
            r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"He"},"finish_reason":null}]}"#,
        ).unwrap())
        .unwrap();
    assert!(matches!(chunks[0], ModelChunk::MessageStart { .. }));
    assert!(matches!(chunks[1], ModelChunk::ContentDelta { .. }));

    let chunks = agg
        .ingest(serde_json::from_str(
            r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"llo"},"finish_reason":null}]}"#,
        ).unwrap())
        .unwrap();
    assert_eq!(chunks.len(), 1);

    let chunks = agg
        .ingest(serde_json::from_str(
            r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#,
        ).unwrap())
        .unwrap();
    assert!(matches!(chunks.last(), Some(ModelChunk::MessageEnd { .. })));
}

#[test]
fn aggregator_assembles_split_tool_call() {
    let mut agg = StreamAggregator::new();
    agg.ingest(serde_json::from_str(
        r#"{"id":"x","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_1","function":{"name":"grep","arguments":""}}]},"finish_reason":null}]}"#,
    ).unwrap()).unwrap();
    agg.ingest(serde_json::from_str(
        r#"{"id":"x","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"q\":\"foo\"}"}}]},"finish_reason":null}]}"#,
    ).unwrap()).unwrap();
    let final_chunks = agg
        .ingest(
            serde_json::from_str(
                r#"{"id":"x","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
            )
            .unwrap(),
        )
        .unwrap();

    let tool_use = final_chunks
        .iter()
        .find_map(|c| match c {
            ModelChunk::ContentDelta {
                delta: ContentBlock::ToolUse { id, name, input },
                ..
            } => Some((id.clone(), name.clone(), input.clone())),
            _ => None,
        })
        .expect("tool use emitted");
    assert_eq!(tool_use.0, ToolCallId("call_1".into()));
    assert_eq!(tool_use.1, "grep");
    assert_eq!(tool_use.2, json!({"q": "foo"}));
}
