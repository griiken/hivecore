//! Agent loop driver. Pi-shape nested-loop with hook gates between steps.
//!
//! ## Invariants
//!
//! - **Append-only message log.** `state.messages` is never mutated; new
//!   entries are pushed. This guarantees the prompt sent to the model on
//!   turn N+1 is an *exact prefix* of turn N's prompt plus new tail items —
//!   the property required for provider prompt caching to land hits.
//!   See: <https://openai.com/index/unrolling-the-codex-agent-loop>
//!   ("Performance considerations" — caching).
//!
//! - **Insert, never edit, on environment changes.** Mid-session updates
//!   (sandbox config, cwd, tool set) must arrive as new `AgentMessage::Custom`
//!   entries rather than mutating earlier messages, for the same reason.
//!
//! - **Compaction belongs in `ContextTransform::transform_outgoing`.** Codex
//!   compacts when token count crosses a threshold; the trait slot exists,
//!   the policy is Layer 3's call.

use std::sync::Arc;

use chrono::Utc;
use hivecore_runtime_core::{
    AbortSignal, AgentEvent, AgentMessage, AgentState, ContentBlock, ContextTransform, HookOutcome,
    LifecycleEvent, LifecycleHook, LifecycleOutcome, MessageId, ModelAdapter, ModelRequest,
    PostHookOutcome, SessionId, StopReason, ToolExecutionMode, ToolHook, ToolHookContext,
    ToolInvocation, ToolOutcome, ToolPostContext, TurnId, UpdateSink,
};

use crate::accumulator::{AssembledTurn, DefaultConsumer, StreamConsumer, ToolCallRequest};
use crate::error::LoopError;
use crate::registry::ToolRegistry;
use crate::sink::EventSink;
use crate::steering::{NoopSteering, SteeringSource};

/// ADR-036 — outcome of `PreCompact` lifecycle dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactGate {
    Proceed,
    Skip,
}

/// Outcome of a single `run` invocation.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub stop_reason: StopReason,
    pub messages_appended: usize,
}

pub struct AgentLoop {
    state: AgentState,
    model: Arc<dyn ModelAdapter>,
    tools: ToolRegistry,
    hooks: Vec<Arc<dyn ToolHook>>,
    lifecycle: Vec<Arc<dyn LifecycleHook>>,
    sink: Arc<dyn EventSink>,
    steering: Arc<dyn SteeringSource>,
    context_transform: Arc<dyn ContextTransform>,
    /// Hard cap on assistant turns per `run` invocation. Catches runaway
    /// tool-call loops without relying on model honesty.
    max_iterations: usize,
}

/// Default `ContextTransform` — pass-through, never compacts.
struct NoopContextTransform;

#[async_trait::async_trait]
impl ContextTransform for NoopContextTransform {}

impl std::fmt::Debug for AgentLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLoop")
            .field("session_id", &self.state.session_id)
            .field("model", &self.model.id())
            .field("tools", &self.tools)
            .field("hooks", &self.hooks.len())
            .field("max_iterations", &self.max_iterations)
            .finish()
    }
}

impl AgentLoop {
    pub fn builder() -> AgentLoopBuilder {
        AgentLoopBuilder::default()
    }

    pub fn state(&self) -> &AgentState {
        &self.state
    }

    /// Inject a user prompt and drive the loop until the model stops or an
    /// abort fires. Steering / follow-up hooks may keep the loop alive past
    /// the natural stop point.
    pub async fn run(
        &mut self,
        prompt: AgentMessage,
        signal: AbortSignal,
    ) -> Result<RunOutcome, LoopError> {
        let baseline = self.state.messages.len();

        self.sink
            .emit(AgentEvent::AgentStart {
                session_id: self.state.session_id,
                at: Utc::now(),
            })
            .await;
        self.fire_lifecycle(LifecycleEvent::AgentStart {
            session_id: self.state.session_id,
        })
        .await?;

        self.state.messages.push(prompt.clone());
        self.fire_lifecycle(LifecycleEvent::PostMessageCommit { message: &prompt })
            .await?;
        self.sink
            .emit(AgentEvent::MessageCommitted { message: prompt })
            .await;

        let mut last_stop;
        let mut iteration = 0usize;

        // Outer loop: re-enters when follow-up messages arrive after a stop.
        'outer: loop {
            // Inner loop: consume model output + tool calls + steering until
            // the model produces a turn with no tool calls and no pending
            // injections.
            loop {
                if iteration >= self.max_iterations {
                    return Err(LoopError::MaxIterations(self.max_iterations));
                }
                iteration += 1;

                if signal.is_aborted() {
                    last_stop = StopReason::Error;
                    break 'outer;
                }

                // Inject any user-typed steering messages before the next
                // assistant turn.
                let steering = self.steering.drain_steering().await;
                for msg in steering {
                    self.state.messages.push(msg);
                }

                let turn_id = TurnId::new();
                self.fire_lifecycle(LifecycleEvent::PreTurn {
                    turn_id,
                    message_count: self.state.messages.len(),
                })
                .await?;
                self.sink
                    .emit(AgentEvent::TurnStart {
                        turn_id,
                        at: Utc::now(),
                    })
                    .await;

                // ADR-026 compaction step. Runs before the model call so the
                // marker is committed first and the request payload reflects
                // the shrunk state.
                self.maybe_compact_step().await?;

                let assembled = self.stream_one_turn(turn_id, signal.clone()).await?;

                let stop_reason = assembled.stop_reason;
                last_stop = stop_reason;
                let tool_calls = assembled.tool_calls.clone();
                let assistant_msg = assembled.message;
                self.state.messages.push(assistant_msg.clone());
                self.fire_lifecycle(LifecycleEvent::PostMessageCommit {
                    message: &assistant_msg,
                })
                .await?;
                self.sink
                    .emit(AgentEvent::MessageCommitted {
                        message: assistant_msg,
                    })
                    .await;

                self.fire_lifecycle(LifecycleEvent::PostTurn {
                    turn_id,
                    stop: stop_reason,
                })
                .await?;
                self.sink
                    .emit(AgentEvent::TurnEnd {
                        turn_id,
                        at: Utc::now(),
                        stop_reason,
                    })
                    .await;

                // Terminal stops abort the inner loop immediately.
                if matches!(
                    stop_reason,
                    StopReason::Error | StopReason::Refusal | StopReason::MaxTokens
                ) {
                    break 'outer;
                }

                // No tool calls + no follow-ups ⇒ inner loop done; check
                // outer stage for follow-ups.
                if tool_calls.is_empty() {
                    break;
                }

                // Run all tool calls; failures inside hooks short-circuit.
                // ADR-035: AND-across-batch terminate hint. If every
                // finalized post-hook returned `ReplaceAndTerminate`, the
                // batch agreed to end the turn — set EndTurn and exit
                // without another model call.
                let terminate = self
                    .dispatch_tools(turn_id, tool_calls, signal.clone())
                    .await?;
                if terminate {
                    last_stop = StopReason::EndTurn;
                    break 'outer;
                }
            }

            // Outer-stage follow-ups (post-stop). If a source supplies more
            // messages, push them and re-enter the inner loop.
            let follow_ups = self.steering.drain_follow_ups().await;
            if follow_ups.is_empty() {
                break 'outer;
            }
            for msg in follow_ups {
                self.state.messages.push(msg);
            }
        }

        self.fire_lifecycle(LifecycleEvent::AgentEnd {
            session_id: self.state.session_id,
            stop: last_stop,
        })
        .await?;
        self.sink
            .emit(AgentEvent::AgentEnd {
                session_id: self.state.session_id,
                at: Utc::now(),
                reason: last_stop,
            })
            .await;

        Ok(RunOutcome {
            stop_reason: last_stop,
            messages_appended: self.state.messages.len() - baseline,
        })
    }

    /// Fan a `LifecycleEvent` out to every registered hook in order. The
    /// first non-`Pass` outcome short-circuits subsequent hooks and either
    /// returns a `LoopError` (Abort / ManualAttention) or logs and
    /// continues (FailedContinue).
    async fn fire_lifecycle(&self, event: LifecycleEvent<'_>) -> Result<(), LoopError> {
        for hook in &self.lifecycle {
            match hook.on_event(event.clone()).await {
                LifecycleOutcome::Pass => {}
                LifecycleOutcome::FailedContinue { reason } => {
                    tracing::warn!(reason, "lifecycle hook reported FailedContinue");
                }
                LifecycleOutcome::FailedAbort { reason } => {
                    return Err(LoopError::HookAborted(reason));
                }
                LifecycleOutcome::ManualAttention { reason } => {
                    return Err(LoopError::ManualAttention(reason));
                }
            }
        }
        Ok(())
    }

    async fn stream_one_turn(
        &self,
        turn_id: TurnId,
        signal: AbortSignal,
    ) -> Result<AssembledTurn, LoopError> {
        // ADR-026 — derive the model payload from `state.messages` via the
        // configured `ContextTransform`. Default impl is pass-through; the
        // `hivecore-compaction::SummarizingTransform` reads the latest
        // `compaction_marker` and rebuilds `[user_msgs, summary, tail]`.
        let payload = self
            .context_transform
            .transform_outgoing(&self.state, self.visible_messages())
            .await
            .map_err(LoopError::Runtime)?;
        let req = ModelRequest {
            model: self.state.model.clone(),
            system: self.state.system_prompt.clone(),
            messages: payload,
            tools: self.tools.descriptors(),
            thinking: self.state.thinking,
            max_tokens: None,
        };
        // Pre-model-request lifecycle gate: budgets, rate limits, dry-run.
        self.fire_lifecycle(LifecycleEvent::PreModelRequest { request: &req })
            .await?;
        let stream = self.model.complete(req, signal).await?;
        let consumer = DefaultConsumer {
            sink: self.sink.as_ref(),
        };
        consumer
            .consume(turn_id, stream)
            .await
            .map_err(LoopError::Runtime)
    }

    /// ADR-026 + ADR-036 step. Asks the configured `ContextTransform`
    /// whether compaction should fire. If `Some(marker)` is returned:
    /// 1. Fires `LifecycleEvent::PreCompact` with the unappended marker —
    ///    a `FailedContinue` outcome cancels this round.
    /// 2. Appends the marker to `state.messages` so the disk log records
    ///    it via the existing `MessageCommitted` path.
    /// 3. Fires `PostMessageCommit` (unchanged) + `PostCompact`.
    async fn maybe_compact_step(&mut self) -> Result<(), LoopError> {
        let marker = self
            .context_transform
            .maybe_compact(&self.state)
            .await
            .map_err(LoopError::Runtime)?;
        let Some(marker) = marker else {
            return Ok(());
        };
        // ADR-036 PreCompact — cancellable via FailedContinue.
        let pre = self
            .fire_lifecycle_compact(LifecycleEvent::PreCompact {
                state: &self.state,
                marker: &marker,
            })
            .await?;
        if pre == CompactGate::Skip {
            return Ok(());
        }
        self.state.messages.push(marker.clone());
        self.fire_lifecycle(LifecycleEvent::PostMessageCommit { message: &marker })
            .await?;
        let marker_id = marker.id().clone();
        self.sink
            .emit(AgentEvent::MessageCommitted { message: marker })
            .await;
        // ADR-036 PostCompact — informational; non-Pass recorded but no
        // unwind (the marker is already on disk).
        self.fire_lifecycle(LifecycleEvent::PostCompact {
            state: &self.state,
            marker_id: &marker_id,
        })
        .await?;
        Ok(())
    }

    /// PreCompact-specific dispatch. Distinguishes `FailedContinue` (skip
    /// compaction this round) from `FailedAbort` / `ManualAttention`
    /// (bubble), which the generic `fire_lifecycle` collapses.
    async fn fire_lifecycle_compact(
        &self,
        event: LifecycleEvent<'_>,
    ) -> Result<CompactGate, LoopError> {
        for hook in &self.lifecycle {
            match hook.on_event(event.clone()).await {
                LifecycleOutcome::Pass => {}
                LifecycleOutcome::FailedContinue { reason } => {
                    tracing::warn!(reason, "PreCompact hook cancelled compaction round");
                    return Ok(CompactGate::Skip);
                }
                LifecycleOutcome::FailedAbort { reason } => {
                    return Err(LoopError::HookAborted(reason));
                }
                LifecycleOutcome::ManualAttention { reason } => {
                    return Err(LoopError::ManualAttention(reason));
                }
            }
        }
        Ok(CompactGate::Proceed)
    }

    fn visible_messages(&self) -> Vec<AgentMessage> {
        self.state
            .messages
            .iter()
            .filter(|m| m.visible_to_model())
            .cloned()
            .collect()
    }

    async fn dispatch_tools(
        &mut self,
        turn_id: TurnId,
        calls: Vec<ToolCallRequest>,
        signal: AbortSignal,
    ) -> Result<bool, LoopError> {
        // ADR-034: partition by `Tool::execution_mode()`. Parallel-eligible
        // calls run concurrently via `futures::future::join_all`; sequential
        // calls run one-at-a-time AFTER the parallel batch settles. Tools
        // not present in the registry default to Sequential (safe — model
        // sees the canonical "tool not found" path through `execute_tool`).
        // ADR-035: AND-across-batch terminate flag.
        if calls.is_empty() {
            return Ok(false);
        }

        // Phase 1 — classify each call. Per-invocation classification
        // (ADR-034 + ADR-029 A3): tools may dispatch differently based
        // on the specific arguments (e.g. MCP `mcp_call` consults its
        // `ToolAnnotations.read_only_hint` cache).
        let mut classified: Vec<(usize, ToolInvocation, ToolExecutionMode)> =
            Vec::with_capacity(calls.len());
        for (idx, call) in calls.into_iter().enumerate() {
            let invocation = ToolInvocation {
                id: call.id,
                name: call.name,
                input: call.input,
            };
            let mode = self
                .tools
                .get(&invocation.name)
                .map(|t| t.execution_mode_for(&invocation))
                .unwrap_or(ToolExecutionMode::Sequential);
            classified.push((idx, invocation, mode));
        }
        let total = classified.len();
        let (parallel, sequential): (Vec<_>, Vec<_>) = classified
            .into_iter()
            .partition(|(_, _, m)| *m == ToolExecutionMode::Parallel);

        // Buffer per-source-index so message emission preserves source order.
        let mut results: Vec<Option<(ToolInvocation, ToolOutcome, bool)>> =
            (0..total).map(|_| None).collect();

        // Phase 2a — parallel batch. ToolExecEnd events fire in completion
        // order (see `process_one_call`); tool_result messages are emitted
        // later in source order on the main thread.
        if !parallel.is_empty() {
            // Reborrow as a shared ref so each closure can capture a Copy
            // of the immutable handle — `process_one_call` only needs
            // `&self`, and parallel futures must share the same borrow.
            let this: &Self = self;
            let futs = parallel.into_iter().map(|(idx, inv, _)| {
                let sig = signal.clone();
                async move {
                    let res = this.process_one_call(turn_id, &inv, sig).await?;
                    Result::<_, LoopError>::Ok((idx, inv, res))
                }
            });
            let outs = futures::future::join_all(futs).await;
            for o in outs {
                let (idx, inv, (outcome, terminate)) = o?;
                results[idx] = Some((inv, outcome, terminate));
            }
        }

        // Phase 2b — sequential batch.
        for (idx, inv, _) in sequential {
            let (outcome, terminate) = self.process_one_call(turn_id, &inv, signal.clone()).await?;
            results[idx] = Some((inv, outcome, terminate));
        }

        // Phase 3 — emit tool_result messages in source order; AND across
        // the terminate flags. (ADR-035: empty batch handled via early
        // return above; here `total > 0` guarantees `any_call`.)
        let mut all_terminate = true;
        for slot in results.into_iter() {
            let (inv, outcome, terminate) = slot.expect("each call has a result");
            if !terminate {
                all_terminate = false;
            }
            let tool_msg = AgentMessage::ToolResult {
                id: MessageId(format!("toolresult-{}", inv.id.0)),
                tool_call_id: inv.id,
                content: outcome.content,
                is_error: outcome.is_error,
            };
            self.state.messages.push(tool_msg.clone());
            self.sink
                .emit(AgentEvent::MessageCommitted { message: tool_msg })
                .await;
        }
        Ok(all_terminate)
    }

    /// Pre-hook + execute + post-hook for a single tool call. Emits
    /// `ToolExecEnd` on completion (source-order emission of the
    /// `tool_result` message + `MessageCommitted` event happens in the
    /// caller, after the whole batch settles).
    async fn process_one_call(
        &self,
        turn_id: TurnId,
        invocation: &ToolInvocation,
        signal: AbortSignal,
    ) -> Result<(ToolOutcome, bool), LoopError> {
        let pre = self.run_pre_hooks(turn_id, invocation).await;
        let outcome = match pre {
            HookDecision::Pass => self.execute_tool(turn_id, invocation, signal).await?,
            HookDecision::Override(o) => o,
            HookDecision::FailedContinue(reason) => ToolOutcome::error(reason),
            HookDecision::FailedAbort(reason) => return Err(LoopError::HookAborted(reason)),
            HookDecision::ManualAttention(reason) => {
                return Err(LoopError::ManualAttention(reason))
            }
        };
        let (final_outcome, terminate) = self.run_post_hooks(turn_id, invocation, outcome).await;
        self.sink
            .emit(AgentEvent::ToolExecEnd {
                tool_call_id: invocation.id.clone(),
                is_error: final_outcome.is_error,
                result: final_outcome
                    .details
                    .clone()
                    .unwrap_or(serde_json::Value::Null),
            })
            .await;
        Ok((final_outcome, terminate))
    }

    async fn run_pre_hooks(&self, turn_id: TurnId, invocation: &ToolInvocation) -> HookDecision {
        for hook in &self.hooks {
            let ctx = ToolHookContext {
                session_id: self.state.session_id,
                turn_id,
                invocation,
            };
            match hook.before(ctx).await {
                HookOutcome::Pass => continue,
                HookOutcome::Override(o) => return HookDecision::Override(o),
                HookOutcome::FailedContinue { reason } => {
                    return HookDecision::FailedContinue(reason)
                }
                HookOutcome::FailedAbort { reason } => return HookDecision::FailedAbort(reason),
                HookOutcome::ManualAttention { reason } => {
                    return HookDecision::ManualAttention(reason)
                }
            }
        }
        HookDecision::Pass
    }

    async fn run_post_hooks(
        &self,
        turn_id: TurnId,
        invocation: &ToolInvocation,
        mut outcome: ToolOutcome,
    ) -> (ToolOutcome, bool) {
        // ADR-035: terminate flag is "last hook wins" — same rule as outcome.
        // Pass leaves the running flag untouched; Replace clears it; only
        // ReplaceAndTerminate sets it.
        let mut terminate = false;
        for hook in &self.hooks {
            let ctx = ToolPostContext {
                session_id: self.state.session_id,
                turn_id,
                invocation,
                outcome: &outcome,
            };
            match hook.after(ctx).await {
                PostHookOutcome::Pass => {}
                PostHookOutcome::Replace(new_outcome) => {
                    outcome = new_outcome;
                    terminate = false;
                }
                PostHookOutcome::ReplaceAndTerminate(new_outcome) => {
                    outcome = new_outcome;
                    terminate = true;
                }
            }
        }
        (outcome, terminate)
    }

    async fn execute_tool(
        &self,
        _turn_id: TurnId,
        invocation: &ToolInvocation,
        signal: AbortSignal,
    ) -> Result<ToolOutcome, LoopError> {
        self.sink
            .emit(AgentEvent::ToolExecStart {
                tool_call_id: invocation.id.clone(),
                name: invocation.name.clone(),
                input: invocation.input.clone(),
            })
            .await;

        let Some(tool) = self.tools.get(&invocation.name) else {
            let outcome = ToolOutcome::error(format!("tool not found: {}", invocation.name));
            return Ok(outcome);
        };

        let sink = self.sink.clone();
        let tool_id = invocation.id.clone();
        let on_update = UpdateSink::new(move |update| {
            let sink = sink.clone();
            let tool_id = tool_id.clone();
            // Fire-and-forget — sinks must not stall the runtime.
            let fut = async move {
                sink.emit(AgentEvent::ToolExecUpdate {
                    tool_call_id: tool_id,
                    update,
                })
                .await;
            };
            // Best-effort: only spawn if a runtime is present.
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(fut);
            }
        });

        match tool.execute(invocation.clone(), signal, on_update).await {
            Ok(outcome) => Ok(outcome),
            Err(err) => Ok(ToolOutcome::error(err.to_string())),
        }
    }
}

#[derive(Debug)]
enum HookDecision {
    Pass,
    Override(ToolOutcome),
    FailedContinue(String),
    FailedAbort(String),
    ManualAttention(String),
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct AgentLoopBuilder {
    session_id: Option<SessionId>,
    system_prompt: Option<String>,
    model_id: Option<String>,
    model: Option<Arc<dyn ModelAdapter>>,
    tools: Option<ToolRegistry>,
    hooks: Vec<Arc<dyn ToolHook>>,
    lifecycle: Vec<Arc<dyn LifecycleHook>>,
    sink: Option<Arc<dyn EventSink>>,
    steering: Option<Arc<dyn SteeringSource>>,
    context_transform: Option<Arc<dyn ContextTransform>>,
    max_iterations: Option<usize>,
    resumed_messages: Vec<AgentMessage>,
}

impl std::fmt::Debug for AgentLoopBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLoopBuilder").finish()
    }
}

impl AgentLoopBuilder {
    pub fn session_id(mut self, id: SessionId) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn system_prompt(mut self, s: impl Into<String>) -> Self {
        self.system_prompt = Some(s.into());
        self
    }

    pub fn model(mut self, model: Arc<dyn ModelAdapter>, model_id: impl Into<String>) -> Self {
        self.model = Some(model);
        self.model_id = Some(model_id.into());
        self
    }

    pub fn tools(mut self, tools: ToolRegistry) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn hook(mut self, hook: Arc<dyn ToolHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    pub fn lifecycle_hook(mut self, hook: Arc<dyn LifecycleHook>) -> Self {
        self.lifecycle.push(hook);
        self
    }

    pub fn sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    pub fn steering(mut self, src: Arc<dyn SteeringSource>) -> Self {
        self.steering = Some(src);
        self
    }

    /// ADR-026 — install a `ContextTransform`. Default is pass-through.
    /// `hivecore-compaction::SummarizingTransform` is the canonical impl.
    pub fn context_transform(mut self, t: Arc<dyn ContextTransform>) -> Self {
        self.context_transform = Some(t);
        self
    }

    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Seed the loop with prior conversation messages — used by
    /// `--continue` / `--session <id>` resume paths. The supplied messages
    /// are pushed into `state.messages` before the first turn, so the next
    /// model call sees the full prior history.
    ///
    /// Note: append-only invariant still holds — `run(prompt)` appends
    /// `prompt` after these resumed messages, never edits them.
    pub fn resume(mut self, messages: Vec<AgentMessage>) -> Self {
        self.resumed_messages = messages;
        self
    }

    pub fn build(self) -> Result<AgentLoop, BuilderError> {
        let model = self.model.ok_or(BuilderError::MissingModel)?;
        let model_id = self.model_id.unwrap_or_else(|| model.id().to_string());
        let sink = self
            .sink
            .unwrap_or_else(|| Arc::new(crate::sink::NoopSink) as Arc<dyn EventSink>);
        let steering = self
            .steering
            .unwrap_or_else(|| Arc::new(NoopSteering) as Arc<dyn SteeringSource>);
        let context_transform = self
            .context_transform
            .unwrap_or_else(|| Arc::new(NoopContextTransform) as Arc<dyn ContextTransform>);

        Ok(AgentLoop {
            state: AgentState {
                session_id: self.session_id.unwrap_or_default(),
                system_prompt: self.system_prompt.unwrap_or_default(),
                model: model_id,
                thinking: hivecore_runtime_core::ThinkingLevel::Off,
                tools: self
                    .tools
                    .as_ref()
                    .map(|t| t.descriptors())
                    .unwrap_or_default(),
                messages: self.resumed_messages,
                is_streaming: false,
            },
            model,
            tools: self.tools.unwrap_or_default(),
            hooks: self.hooks,
            lifecycle: self.lifecycle,
            sink,
            steering,
            context_transform,
            max_iterations: self.max_iterations.unwrap_or(64),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("model adapter is required")]
    MissingModel,
}

// Convenience: build a user-text message.
pub fn user_text(text: impl Into<String>) -> AgentMessage {
    AgentMessage::User {
        id: MessageId(format!("user-{}", uuid::Uuid::new_v4())),
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
