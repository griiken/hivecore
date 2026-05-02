use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_stream::try_stream;
use async_trait::async_trait;
use futures::Stream;
use hivecore_runtime_core::{
    AbortSignal, AgentEvent, ContentBlock, LifecycleEvent, LifecycleHook, LifecycleOutcome,
    MessageId, ModelAdapter, ModelChunk, ModelRequest, ModelStream, RuntimeError, RuntimeResult,
    StopReason, TokenUsage, Tool, ToolCallId, ToolInvocation, ToolOutcome, UpdateSink,
};

use super::*;
use crate::sink::VecSink;

// -- Mock model that replays a fixed script of chunk sequences ------------
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

// -- Mock tool that records inputs and returns canned output --------------
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "echo the input"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"text":{"type":"string"}}})
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let text = invocation
            .input
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(ToolOutcome::ok(vec![ContentBlock::Text { text }]))
    }
}

fn mid(s: &str) -> MessageId {
    MessageId(s.into())
}

fn end_chunk(stop: StopReason) -> ModelChunk {
    ModelChunk::MessageEnd {
        id: mid("m"),
        stop_reason: stop,
        usage: TokenUsage::default(),
    }
}

#[tokio::test]
async fn single_turn_text_only() {
    let model = Arc::new(ScriptedModel::new(vec![vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::Text {
                text: "hi user".into(),
            },
        },
        end_chunk(StopReason::EndTurn),
    ]]));

    let sink = Arc::new(VecSink::new());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-1")
        .sink(sink.clone())
        .build()
        .unwrap();

    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("hello"), sig).await.unwrap();

    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    assert_eq!(outcome.messages_appended, 2); // user prompt + assistant
    let events = sink.snapshot();
    assert!(matches!(
        events.first(),
        Some(AgentEvent::AgentStart { .. })
    ));
    assert!(matches!(events.last(), Some(AgentEvent::AgentEnd { .. })));
}

#[tokio::test]
async fn tool_call_round_trip() {
    // Push scripts in reverse — pop() returns LIFO.
    let model = Arc::new(ScriptedModel::new(vec![
        // Second turn: model sees tool result, replies plain text.
        vec![
            ModelChunk::MessageStart { id: mid("m2") },
            ModelChunk::ContentDelta {
                id: mid("m2"),
                delta: ContentBlock::Text {
                    text: "done".into(),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m2"),
                stop_reason: StopReason::EndTurn,
                usage: TokenUsage::default(),
            },
        ],
        // First turn: emit a tool call.
        vec![
            ModelChunk::MessageStart { id: mid("m1") },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("call-1".into()),
                    name: "echo".into(),
                    input: serde_json::json!({"text": "ping"}),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m1"),
                stop_reason: StopReason::ToolUse,
                usage: TokenUsage::default(),
            },
        ],
    ]));

    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let sink = Arc::new(VecSink::new());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-2")
        .tools(tools)
        .sink(sink.clone())
        .build()
        .unwrap();

    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("ping"), sig).await.unwrap();

    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    // user + asst-1(toolUse) + tool_result + asst-2(text) = 4
    assert_eq!(outcome.messages_appended, 4);

    let events = sink.snapshot();
    let exec_starts = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolExecStart { .. }))
        .count();
    let exec_ends = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolExecEnd { .. }))
        .count();
    assert_eq!(exec_starts, 1);
    assert_eq!(exec_ends, 1);
    let turn_starts = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::TurnStart { .. }))
        .count();
    assert_eq!(turn_starts, 2);
}

#[tokio::test]
async fn unknown_tool_returns_error_result_without_aborting() {
    let model = Arc::new(ScriptedModel::new(vec![
        // Second turn: stop after seeing the error tool result.
        vec![
            ModelChunk::MessageStart { id: mid("m2") },
            ModelChunk::ContentDelta {
                id: mid("m2"),
                delta: ContentBlock::Text { text: "ok".into() },
            },
            ModelChunk::MessageEnd {
                id: mid("m2"),
                stop_reason: StopReason::EndTurn,
                usage: TokenUsage::default(),
            },
        ],
        // First turn: call a tool that doesn't exist.
        vec![
            ModelChunk::MessageStart { id: mid("m1") },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c1".into()),
                    name: "nonexistent".into(),
                    input: serde_json::json!({}),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m1"),
                stop_reason: StopReason::ToolUse,
                usage: TokenUsage::default(),
            },
        ],
    ]));

    let mut agent = AgentLoop::builder()
        .model(model, "scripted-3")
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("go"), sig).await.unwrap();
    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
}

#[tokio::test]
async fn max_iterations_caps_runaway_loops() {
    // Model emits tool call forever.
    let mut scripts = Vec::new();
    for _ in 0..100 {
        scripts.push(vec![
            ModelChunk::MessageStart { id: mid("m") },
            ModelChunk::ContentDelta {
                id: mid("m"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c".into()),
                    name: "echo".into(),
                    input: serde_json::json!({"text":"x"}),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m"),
                stop_reason: StopReason::ToolUse,
                usage: TokenUsage::default(),
            },
        ]);
    }
    let model = Arc::new(ScriptedModel::new(scripts));
    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-4")
        .tools(tools)
        .max_iterations(3)
        .build()
        .unwrap();

    let (_h, sig) = AbortSignal::new();
    let err = agent.run(user_text("loop"), sig).await.unwrap_err();
    assert!(matches!(err, LoopError::MaxIterations(3)));
}

#[tokio::test]
async fn aborted_signal_breaks_loop() {
    let model = Arc::new(ScriptedModel::new(vec![vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::Text { text: "hi".into() },
        },
        end_chunk(StopReason::EndTurn),
    ]]));

    let mut agent = AgentLoop::builder()
        .model(model, "scripted-5")
        .build()
        .unwrap();

    let (handle, sig) = AbortSignal::new();
    handle.abort();
    let outcome = agent.run(user_text("x"), sig).await.unwrap();
    // Aborted before any turn ⇒ Error stop reason, no assistant message.
    assert_eq!(outcome.stop_reason, StopReason::Error);
}

// -- Lifecycle hooks ------------------------------------------------------

#[derive(Debug, Default)]
struct RecordingLifecycle {
    seen: Mutex<Vec<&'static str>>,
}

#[async_trait]
impl LifecycleHook for RecordingLifecycle {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        let tag = match event {
            LifecycleEvent::AgentStart { .. } => "agent_start",
            LifecycleEvent::PreTurn { .. } => "pre_turn",
            LifecycleEvent::PreModelRequest { .. } => "pre_model_request",
            LifecycleEvent::PostMessageCommit { .. } => "post_message_commit",
            LifecycleEvent::PostTurn { .. } => "post_turn",
            LifecycleEvent::AgentEnd { .. } => "agent_end",
        };
        self.seen.lock().unwrap().push(tag);
        LifecycleOutcome::Pass
    }
}

#[tokio::test]
async fn lifecycle_hook_fires_at_every_checkpoint() {
    let model = Arc::new(ScriptedModel::new(vec![vec![
        ModelChunk::MessageStart { id: mid("m") },
        ModelChunk::ContentDelta {
            id: mid("m"),
            delta: ContentBlock::Text { text: "hi".into() },
        },
        end_chunk(StopReason::EndTurn),
    ]]));

    let recorder = Arc::new(RecordingLifecycle::default());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-life-1")
        .lifecycle_hook(recorder.clone() as Arc<dyn LifecycleHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    agent.run(user_text("ping"), sig).await.unwrap();

    let seen = recorder.seen.lock().unwrap().clone();
    // user prompt + one assistant turn:
    //   agent_start, post_message_commit (user), pre_turn, pre_model_request,
    //   post_message_commit (assistant), post_turn, agent_end
    assert_eq!(
        seen,
        vec![
            "agent_start",
            "post_message_commit",
            "pre_turn",
            "pre_model_request",
            "post_message_commit",
            "post_turn",
            "agent_end",
        ]
    );
}

#[derive(Debug, Default)]
struct AbortAtPreTurn;

#[async_trait]
impl LifecycleHook for AbortAtPreTurn {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        if matches!(event, LifecycleEvent::PreTurn { .. }) {
            LifecycleOutcome::FailedAbort {
                reason: "budget exceeded".into(),
            }
        } else {
            LifecycleOutcome::Pass
        }
    }
}

#[tokio::test]
async fn lifecycle_hook_failed_abort_stops_loop() {
    let model = Arc::new(ScriptedModel::new(vec![vec![
        ModelChunk::MessageStart { id: mid("m") },
        end_chunk(StopReason::EndTurn),
    ]]));

    let mut agent = AgentLoop::builder()
        .model(model, "scripted-life-2")
        .lifecycle_hook(Arc::new(AbortAtPreTurn) as Arc<dyn LifecycleHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let err = agent.run(user_text("x"), sig).await.unwrap_err();
    assert!(matches!(
        err,
        crate::error::LoopError::HookAborted(ref s) if s == "budget exceeded"
    ));
}
