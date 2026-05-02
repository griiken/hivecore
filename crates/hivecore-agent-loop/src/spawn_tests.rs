use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_stream::try_stream;
use async_trait::async_trait;
use futures::Stream;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, MessageId, ModelAdapter, ModelChunk, ModelRequest, ModelStream,
    RuntimeError, RuntimeResult, StopReason, TokenUsage,
};

use super::*;
use crate::driver::AgentLoop;

struct ScriptedModel {
    scripts: Mutex<Vec<Vec<ModelChunk>>>,
}

impl ScriptedModel {
    fn new(scripts: Vec<Vec<ModelChunk>>) -> Self {
        Self {
            scripts: Mutex::new(scripts),
        }
    }
}

#[async_trait]
impl ModelAdapter for ScriptedModel {
    fn id(&self) -> &str {
        "scripted"
    }
    async fn complete(
        &self,
        _req: ModelRequest,
        _signal: AbortSignal,
    ) -> RuntimeResult<ModelStream> {
        let chunks = self
            .scripts
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| RuntimeError::ModelFailed("script exhausted".into()))?;
        let stream: Pin<Box<dyn Stream<Item = Result<ModelChunk, RuntimeError>> + Send>> =
            Box::pin(try_stream! {
                for c in chunks {
                    yield c;
                }
            });
        Ok(stream)
    }
}

fn one_text_turn(text: &str) -> Vec<ModelChunk> {
    vec![
        ModelChunk::MessageStart {
            id: MessageId("m".into()),
        },
        ModelChunk::ContentDelta {
            id: MessageId("m".into()),
            delta: ContentBlock::Text { text: text.into() },
        },
        ModelChunk::MessageEnd {
            id: MessageId("m".into()),
            stop_reason: StopReason::EndTurn,
            usage: TokenUsage::default(),
        },
    ]
}

#[tokio::test]
async fn spawns_child_and_returns_assistant_text() {
    let child_model: Arc<dyn ModelAdapter> =
        Arc::new(ScriptedModel::new(vec![one_text_turn("child reply: 42")]));

    let spec: Arc<dyn SubAgentSpec> = Arc::new(ClosureSpec::new(
        "researcher",
        "answers numeric questions",
        move || {
            AgentLoop::builder()
                .system_prompt("be brief")
                .model(child_model.clone(), "scripted-child")
                .build()
                .map_err(|e| RuntimeError::ToolFailed(e.to_string()))
        },
    ));

    let tool = SpawnAgentTool::new(vec![spec]);
    assert!(tool.description().contains("researcher"));

    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            ToolInvocation {
                id: hivecore_runtime_core::ToolCallId("c1".into()),
                name: "spawn_agent".into(),
                input: serde_json::json!({
                    "persona": "researcher",
                    "prompt": "what is 6 * 7?"
                }),
            },
            sig,
            hivecore_runtime_core::UpdateSink::noop(),
        )
        .await
        .unwrap();

    let txt = match &out.content[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert_eq!(txt, "child reply: 42");
    let details = out.details.unwrap();
    assert_eq!(details["persona"], "researcher");
}

#[tokio::test]
async fn rejects_unknown_persona() {
    let model: Arc<dyn ModelAdapter> = Arc::new(ScriptedModel::new(vec![one_text_turn("x")]));
    let spec: Arc<dyn SubAgentSpec> = Arc::new(ClosureSpec::new("alpha", "x", move || {
        AgentLoop::builder()
            .model(model.clone(), "m")
            .build()
            .map_err(|e| RuntimeError::ToolFailed(e.to_string()))
    }));
    let tool = SpawnAgentTool::new(vec![spec]);
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            ToolInvocation {
                id: hivecore_runtime_core::ToolCallId("c1".into()),
                name: "spawn_agent".into(),
                input: serde_json::json!({"persona": "ghost", "prompt": "x"}),
            },
            sig,
            hivecore_runtime_core::UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("unknown sub-agent"));
}
