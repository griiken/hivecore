use std::pin::Pin;

use async_stream::try_stream;
use futures::Stream;
use hivecore_runtime_core::{
    AgentMessage, ContentBlock, MessageId, ModelChunk, RuntimeError, StopReason, TokenUsage,
    ToolCallId, TurnId,
};

use super::*;
use crate::sink::{NoopSink, VecSink};

fn stream_of(
    chunks: Vec<ModelChunk>,
) -> Pin<Box<dyn Stream<Item = Result<ModelChunk, RuntimeError>> + Send>> {
    Box::pin(try_stream! {
        for c in chunks {
            yield c;
        }
    })
}

fn mid(s: &str) -> MessageId {
    MessageId(s.into())
}

#[tokio::test]
async fn assembles_text_only_turn() {
    let sink = NoopSink;
    let consumer = DefaultConsumer { sink: &sink };
    let chunks = vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::Text { text: "hi".into() },
        },
        ModelChunk::MessageEnd {
            id: mid("m"),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::default(),
        },
    ];

    let out = consumer
        .consume(TurnId::new(), stream_of(chunks))
        .await
        .unwrap();
    assert_eq!(out.stop_reason, StopReason::EndTurn);
    assert!(out.tool_calls.is_empty());
    match out.message {
        AgentMessage::Assistant { content, .. } => {
            assert_eq!(content.len(), 1);
            assert!(matches!(content[0], ContentBlock::Text { .. }));
        }
        _ => panic!("expected assistant"),
    }
}

#[tokio::test]
async fn extracts_tool_calls() {
    let sink = NoopSink;
    let consumer = DefaultConsumer { sink: &sink };
    let chunks = vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::ToolUse {
                id: ToolCallId("t1".into()),
                name: "grep".into(),
                input: serde_json::json!({"q": "foo"}),
            },
        },
        ModelChunk::MessageEnd {
            id: mid("m"),
            stop_reason: StopReason::ToolUse,
            usage: TokenUsage {
                input: 5,
                output: 1,
                ..Default::default()
            },
        },
    ];

    let out = consumer
        .consume(TurnId::new(), stream_of(chunks))
        .await
        .unwrap();
    assert_eq!(out.stop_reason, StopReason::ToolUse);
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_calls[0].name, "grep");
    assert_eq!(out.usage.output, 1);
}

#[tokio::test]
async fn forwards_events_to_sink() {
    let sink = VecSink::new();
    let consumer = DefaultConsumer { sink: &sink };
    let chunks = vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::Text { text: "hi".into() },
        },
        ModelChunk::MessageEnd {
            id: mid("m"),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::default(),
        },
    ];

    consumer
        .consume(TurnId::new(), stream_of(chunks))
        .await
        .unwrap();
    assert_eq!(sink.len(), 3);
}
