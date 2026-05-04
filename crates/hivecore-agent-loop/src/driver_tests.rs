use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_stream::try_stream;
use async_trait::async_trait;
use futures::Stream;
use hivecore_runtime_core::{
    AbortSignal, AgentEvent, AgentMessage, AgentState, ContentBlock, ContextTransform, HookOutcome,
    LifecycleEvent, LifecycleHook, LifecycleOutcome, MessageId, ModelAdapter, ModelChunk,
    ModelRequest, ModelStream, PostHookOutcome, RuntimeError, RuntimeResult, StopReason,
    TokenUsage, Tool, ToolCallId, ToolExecutionMode, ToolHook, ToolHookContext, ToolInvocation,
    ToolOutcome, ToolPostContext, UpdateSink,
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
            LifecycleEvent::PreCompact { .. } => "pre_compact",
            LifecycleEvent::PostCompact { .. } => "post_compact",
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

// -- ADR-036: PreCompact / PostCompact lifecycle events ------------------

/// Synthetic transform that fires compaction once after the first turn.
/// Marker is a `Custom { kind: "compaction_marker", visible_to_model: false }`.
struct OneShotCompact {
    fired: Mutex<bool>,
}

impl std::fmt::Debug for OneShotCompact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneShotCompact").finish()
    }
}

#[async_trait]
impl ContextTransform for OneShotCompact {
    async fn maybe_compact(&self, state: &AgentState) -> RuntimeResult<Option<AgentMessage>> {
        let mut g = self.fired.lock().unwrap();
        // Need at least the user prompt + first assistant turn before firing.
        if *g || state.messages.len() < 2 {
            return Ok(None);
        }
        *g = true;
        Ok(Some(AgentMessage::Custom {
            id: MessageId("compact-marker-1".into()),
            kind: "compaction_marker".into(),
            payload: serde_json::json!({"trigger":"manual"}),
            visible_to_model: false,
        }))
    }
}

#[derive(Debug, Default)]
struct CaptureCompactEvents {
    seen: Mutex<Vec<&'static str>>,
}

#[async_trait]
impl LifecycleHook for CaptureCompactEvents {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        match event {
            LifecycleEvent::PreCompact { .. } => {
                self.seen.lock().unwrap().push("pre_compact");
            }
            LifecycleEvent::PostCompact { .. } => {
                self.seen.lock().unwrap().push("post_compact");
            }
            _ => {}
        }
        LifecycleOutcome::Pass
    }
}

#[derive(Debug, Default)]
struct VetoPreCompact;

#[async_trait]
impl LifecycleHook for VetoPreCompact {
    async fn on_event(&self, event: LifecycleEvent<'_>) -> LifecycleOutcome {
        if matches!(event, LifecycleEvent::PreCompact { .. }) {
            LifecycleOutcome::FailedContinue {
                reason: "skip this round".into(),
            }
        } else {
            LifecycleOutcome::Pass
        }
    }
}

fn two_text_turns() -> Vec<Vec<ModelChunk>> {
    vec![
        vec![
            ModelChunk::MessageStart { id: mid("m2") },
            ModelChunk::ContentDelta {
                id: mid("m2"),
                delta: ContentBlock::Text { text: "b".into() },
            },
            end_chunk(StopReason::EndTurn),
        ],
        vec![
            ModelChunk::MessageStart { id: mid("m1") },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::Text { text: "a".into() },
            },
            end_chunk(StopReason::EndTurn),
        ],
    ]
}

#[tokio::test]
async fn pre_and_post_compact_fire_in_order() {
    let model = Arc::new(ScriptedModel::new(two_text_turns()));
    let transform = Arc::new(OneShotCompact {
        fired: Mutex::new(false),
    });
    let recorder = Arc::new(CaptureCompactEvents::default());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-compact-1")
        .context_transform(transform.clone() as Arc<dyn ContextTransform>)
        .lifecycle_hook(recorder.clone() as Arc<dyn LifecycleHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    // First user prompt produces "a"; second prompt comes via steering — but
    // we have no steering source, so just run twice.
    agent.run(user_text("first"), sig.clone()).await.unwrap();
    agent.run(user_text("second"), sig).await.unwrap();

    let seen = recorder.seen.lock().unwrap().clone();
    // Compaction fires exactly once on the second run (after >=2 messages).
    assert_eq!(seen, vec!["pre_compact", "post_compact"]);
}

#[tokio::test]
async fn pre_compact_failed_continue_skips_round() {
    let model = Arc::new(ScriptedModel::new(two_text_turns()));
    let transform = Arc::new(OneShotCompact {
        fired: Mutex::new(false),
    });
    let recorder = Arc::new(CaptureCompactEvents::default());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-compact-2")
        .context_transform(transform.clone() as Arc<dyn ContextTransform>)
        .lifecycle_hook(Arc::new(VetoPreCompact) as Arc<dyn LifecycleHook>)
        .lifecycle_hook(recorder.clone() as Arc<dyn LifecycleHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    agent.run(user_text("first"), sig.clone()).await.unwrap();
    agent.run(user_text("second"), sig).await.unwrap();

    let seen = recorder.seen.lock().unwrap().clone();
    // Veto runs first → recorder never sees post_compact, but it SHOULD see
    // pre_compact (the first hook fired before the veto-aborter, since
    // hooks run in registration order).
    //
    // Wait — VetoPreCompact returns FailedContinue, which short-circuits
    // *the rest of the lifecycle dispatch* for that event. So recorder
    // never sees pre_compact either. Either way, no post_compact.
    assert!(!seen.contains(&"post_compact"));
}

// -- ADR-034: parallel tool dispatch --------------------------------------

/// Tool that sleeps `delay_ms` then echoes a tag. Used to verify parallel
/// dispatch genuinely overlaps work.
#[derive(Debug)]
struct SleepyTool {
    name: &'static str,
    mode: ToolExecutionMode,
    delay_ms: u64,
}

#[async_trait]
impl Tool for SleepyTool {
    fn name(&self) -> &str {
        self.name
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        self.mode
    }
    fn description(&self) -> &str {
        "sleep then return"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{}})
    }
    async fn execute(
        &self,
        _invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        Ok(ToolOutcome::ok(vec![ContentBlock::Text {
            text: self.name.to_string(),
        }]))
    }
}

fn three_parallel_calls_then_text() -> Vec<Vec<ModelChunk>> {
    vec![
        vec![
            ModelChunk::MessageStart { id: mid("m2") },
            ModelChunk::ContentDelta {
                id: mid("m2"),
                delta: ContentBlock::Text { text: "ok".into() },
            },
            end_chunk(StopReason::EndTurn),
        ],
        vec![
            ModelChunk::MessageStart { id: mid("m1") },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c-a".into()),
                    name: "alpha".into(),
                    input: serde_json::json!({}),
                },
            },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c-b".into()),
                    name: "beta".into(),
                    input: serde_json::json!({}),
                },
            },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c-c".into()),
                    name: "gamma".into(),
                    input: serde_json::json!({}),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m1"),
                stop_reason: StopReason::ToolUse,
                usage: TokenUsage::default(),
            },
        ],
    ]
}

#[tokio::test]
async fn parallel_batch_runs_concurrently() {
    let model = Arc::new(ScriptedModel::new(three_parallel_calls_then_text()));
    let tools = ToolRegistry::new(vec![
        Arc::new(SleepyTool {
            name: "alpha",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 80,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "beta",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 80,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "gamma",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 80,
        }) as Arc<dyn Tool>,
    ]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-par-1")
        .tools(tools)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let start = std::time::Instant::now();
    let _ = agent.run(user_text("go"), sig).await.unwrap();
    let elapsed = start.elapsed();
    // Sequential would take >= 240ms (3 * 80ms). Parallel should land
    // closer to the single-tool delay. Use a generous bound that still
    // distinguishes parallel from sequential.
    assert!(
        elapsed < std::time::Duration::from_millis(220),
        "parallel batch took {elapsed:?}, expected < 220ms"
    );
}

#[tokio::test]
async fn sequential_batch_runs_one_at_a_time() {
    let model = Arc::new(ScriptedModel::new(three_parallel_calls_then_text()));
    let tools = ToolRegistry::new(vec![
        Arc::new(SleepyTool {
            name: "alpha",
            mode: ToolExecutionMode::Sequential,
            delay_ms: 60,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "beta",
            mode: ToolExecutionMode::Sequential,
            delay_ms: 60,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "gamma",
            mode: ToolExecutionMode::Sequential,
            delay_ms: 60,
        }) as Arc<dyn Tool>,
    ]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-seq-1")
        .tools(tools)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let start = std::time::Instant::now();
    let _ = agent.run(user_text("go"), sig).await.unwrap();
    let elapsed = start.elapsed();
    // Sequential ⇒ ~3 * 60ms = 180ms minimum.
    assert!(
        elapsed >= std::time::Duration::from_millis(170),
        "sequential batch took {elapsed:?}, expected >= 170ms"
    );
}

#[tokio::test]
async fn tool_result_messages_emit_in_source_order() {
    // First tool delays longer than the second; with parallel dispatch
    // its execute returns AFTER the second's. The tool_result messages
    // must still emit in source order so the model sees a deterministic
    // log on the next turn.
    let model = Arc::new(ScriptedModel::new(three_parallel_calls_then_text()));
    let tools = ToolRegistry::new(vec![
        Arc::new(SleepyTool {
            name: "alpha",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 100,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "beta",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 50,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "gamma",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 10,
        }) as Arc<dyn Tool>,
    ]);
    let sink = Arc::new(VecSink::new());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-order-1")
        .tools(tools)
        .sink(sink.clone())
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    agent.run(user_text("go"), sig).await.unwrap();

    let events = sink.snapshot();
    let tool_result_order: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::MessageCommitted {
                message: AgentMessage::ToolResult { tool_call_id, .. },
            } => Some(tool_call_id.0.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tool_result_order, vec!["c-a", "c-b", "c-c"]);
}

#[tokio::test]
async fn mixed_modes_still_complete_all_calls() {
    // alpha = Parallel, beta = Sequential, gamma = Parallel. All must run
    // and produce tool_result messages in source order.
    let model = Arc::new(ScriptedModel::new(three_parallel_calls_then_text()));
    let tools = ToolRegistry::new(vec![
        Arc::new(SleepyTool {
            name: "alpha",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 20,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "beta",
            mode: ToolExecutionMode::Sequential,
            delay_ms: 20,
        }) as Arc<dyn Tool>,
        Arc::new(SleepyTool {
            name: "gamma",
            mode: ToolExecutionMode::Parallel,
            delay_ms: 20,
        }) as Arc<dyn Tool>,
    ]);
    let sink = Arc::new(VecSink::new());
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-mixed-1")
        .tools(tools)
        .sink(sink.clone())
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    agent.run(user_text("go"), sig).await.unwrap();

    let events = sink.snapshot();
    let exec_ends = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolExecEnd { .. }))
        .count();
    assert_eq!(exec_ends, 3);
    let tool_result_ids: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::MessageCommitted {
                message: AgentMessage::ToolResult { tool_call_id, .. },
            } => Some(tool_call_id.0.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tool_result_ids, vec!["c-a", "c-b", "c-c"]);
}

// -- ADR-035: PostHookOutcome::ReplaceAndTerminate -----------------------

/// Hook that always returns `ReplaceAndTerminate` from `after`.
#[derive(Debug)]
struct AlwaysTerminate;

#[async_trait]
impl ToolHook for AlwaysTerminate {
    async fn before(&self, _ctx: ToolHookContext<'_>) -> HookOutcome {
        HookOutcome::Pass
    }
    async fn after(&self, ctx: ToolPostContext<'_>) -> PostHookOutcome {
        PostHookOutcome::ReplaceAndTerminate(ctx.outcome.clone())
    }
}

/// Hook whose `after` returns Replace (no terminate). Used to verify the
/// AND-across-batch rule: a single dissenter keeps the loop alive even if
/// other hooks set terminate.
#[derive(Debug)]
struct AlwaysReplaceNoTerminate;

#[async_trait]
impl ToolHook for AlwaysReplaceNoTerminate {
    async fn before(&self, _ctx: ToolHookContext<'_>) -> HookOutcome {
        HookOutcome::Pass
    }
    async fn after(&self, ctx: ToolPostContext<'_>) -> PostHookOutcome {
        PostHookOutcome::Replace(ctx.outcome.clone())
    }
}

fn one_tool_call_then_text() -> Vec<Vec<ModelChunk>> {
    vec![
        // Second turn (only reached if loop continues): plain text + EndTurn.
        vec![
            ModelChunk::MessageStart { id: mid("m2") },
            ModelChunk::ContentDelta {
                id: mid("m2"),
                delta: ContentBlock::Text {
                    text: "after".into(),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m2"),
                stop_reason: StopReason::EndTurn,
                usage: TokenUsage::default(),
            },
        ],
        // First turn: emits one tool call.
        vec![
            ModelChunk::MessageStart { id: mid("m1") },
            ModelChunk::ContentDelta {
                id: mid("m1"),
                delta: ContentBlock::ToolUse {
                    id: ToolCallId("c1".into()),
                    name: "echo".into(),
                    input: serde_json::json!({"text": "x"}),
                },
            },
            ModelChunk::MessageEnd {
                id: mid("m1"),
                stop_reason: StopReason::ToolUse,
                usage: TokenUsage::default(),
            },
        ],
    ]
}

#[tokio::test]
async fn replace_and_terminate_stops_after_tool_batch() {
    let model = Arc::new(ScriptedModel::new(one_tool_call_then_text()));
    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-term-1")
        .tools(tools)
        .hook(Arc::new(AlwaysTerminate) as Arc<dyn ToolHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("go"), sig).await.unwrap();

    // Loop must end at EndTurn after the first tool batch — model's second
    // turn must not be consumed.
    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    // user + asst-1(toolUse) + tool_result = 3 (NO asst-2 from second script)
    assert_eq!(outcome.messages_appended, 3);
}

#[tokio::test]
async fn replace_alone_does_not_terminate() {
    let model = Arc::new(ScriptedModel::new(one_tool_call_then_text()));
    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-term-2")
        .tools(tools)
        .hook(Arc::new(AlwaysReplaceNoTerminate) as Arc<dyn ToolHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("go"), sig).await.unwrap();

    // Loop must continue and consume the second model script.
    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    assert_eq!(outcome.messages_appended, 4);
}

#[tokio::test]
async fn last_hook_wins_terminate_after_replace() {
    // Hook order: Replace (no terminate), then ReplaceAndTerminate.
    // Last wins ⇒ terminate.
    let model = Arc::new(ScriptedModel::new(one_tool_call_then_text()));
    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-term-3")
        .tools(tools)
        .hook(Arc::new(AlwaysReplaceNoTerminate) as Arc<dyn ToolHook>)
        .hook(Arc::new(AlwaysTerminate) as Arc<dyn ToolHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("go"), sig).await.unwrap();
    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    assert_eq!(outcome.messages_appended, 3);
}

#[tokio::test]
async fn last_hook_wins_replace_after_terminate() {
    // Hook order: ReplaceAndTerminate, then Replace (clears terminate).
    // Last wins ⇒ no terminate; loop continues.
    let model = Arc::new(ScriptedModel::new(one_tool_call_then_text()));
    let tools = ToolRegistry::new(vec![Arc::new(EchoTool) as Arc<dyn Tool>]);
    let mut agent = AgentLoop::builder()
        .model(model, "scripted-term-4")
        .tools(tools)
        .hook(Arc::new(AlwaysTerminate) as Arc<dyn ToolHook>)
        .hook(Arc::new(AlwaysReplaceNoTerminate) as Arc<dyn ToolHook>)
        .build()
        .unwrap();
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text("go"), sig).await.unwrap();
    assert_eq!(outcome.stop_reason, StopReason::EndTurn);
    assert_eq!(outcome.messages_appended, 4);
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
