//! `hivecore-acp` — Agent Client Protocol stdio server.
//!
//! Spawned as a subprocess by an ACP-aware editor (e.g. Zed) over stdin/stdout.
//! Wraps the hivecore agent loop + builtin tools.

use std::sync::Arc;

use agent_client_protocol::schema::{
    AgentCapabilities, ContentBlock, ContentChunk, InitializeRequest, InitializeResponse,
    NewSessionRequest, NewSessionResponse, PermissionOption, PermissionOptionId,
    PermissionOptionKind, PromptRequest, PromptResponse, RequestPermissionOutcome,
    RequestPermissionRequest, SelectedPermissionOutcome, SessionId, SessionNotification,
    SessionUpdate, StopReason as AcpStopReason, ToolCall, ToolCallId, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo, Dispatch};
use async_trait::async_trait;
use hivecore_acp_server::bridge::{
    AcpEventSink, AcpPermissionDecision, AcpPrompter, NotificationSender, PermissionRequester,
};
use hivecore_acp_server::server::{run_prompt_turn, ServerConfig, ServerState};
use hivecore_runtime_core::{AbortSignal, Approval, EventSink, ToolHook};
use hivecore_tool_policy::{ApprovalHook, ApprovalPolicy, ToolNameMatcher};
use serde_json::Value;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // never pollute stdout — that's the ACP wire
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = ServerConfig::from_env()?;
    let state = ServerState::new(config)?;

    Agent
        .builder()
        .name("hivecore-acp")
        .on_receive_request(
            async |req: InitializeRequest, responder, _cx| {
                responder.respond(
                    InitializeResponse::new(req.protocol_version)
                        .agent_capabilities(AgentCapabilities::new()),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async |_req: NewSessionRequest, responder, _cx| {
                let id = SessionId::new(uuid::Uuid::new_v4().to_string());
                responder.respond(NewSessionResponse::new(id))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            {
                let state = state.clone();
                async move |req: PromptRequest, responder, cx: ConnectionTo<_>| {
                    let session_id_str = req.session_id.0.to_string();
                    let prompt_text = extract_text(&req.prompt);

                    let conn_sender = ConnectionSender::new(cx.clone(), req.session_id.clone());
                    let sender: Arc<dyn NotificationSender> = Arc::new(conn_sender.clone());
                    let sink: Arc<dyn EventSink> =
                        Arc::new(AcpEventSink::new(session_id_str.clone(), sender));

                    // ADR-029 — install HITL approval unless `HIVECORE_NO_APPROVAL=1`.
                    // Default-on for security; editors that auto-approve everything
                    // can opt out via env.
                    let approval_disabled = std::env::var("HIVECORE_NO_APPROVAL")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false);
                    let mut hooks: Vec<Arc<dyn ToolHook>> = Vec::new();
                    if !approval_disabled {
                        let prompter: Arc<dyn PermissionRequester> = Arc::new(conn_sender.clone());
                        let approval: Arc<dyn Approval> =
                            Arc::new(AcpPrompter::new(session_id_str.clone(), prompter));
                        hooks.push(Arc::new(ApprovalHook::new(
                            ApprovalPolicy::OnRequest,
                            Arc::new(ToolNameMatcher::coder_defaults()),
                            approval,
                        )));
                    }

                    let agent = match state.ensure_session(&session_id_str, sink, hooks).await {
                        Ok(a) => a,
                        Err(e) => {
                            tracing::error!(error = %e, "ensure_session failed");
                            return responder.respond(PromptResponse::new(AcpStopReason::Refusal));
                        }
                    };

                    let (_handle, signal) = AbortSignal::new();
                    // ADR-029 — must spawn the prompt turn so `block_task()`
                    // calls inside `AcpPrompter::request` (which sends an ACP
                    // counter-request via `cx.send_request`) don't deadlock
                    // the dispatch loop. Per
                    // `agent-client-protocol-0.11/jsonrpc.rs:2881-2906`:
                    // counter-requests are only safe in spawned tasks.
                    cx.spawn(async move {
                        let acp_stop = match run_prompt_turn(agent, prompt_text, signal).await {
                            Ok(stop) => match stop {
                                hivecore_runtime_core::StopReason::EndTurn
                                | hivecore_runtime_core::StopReason::ToolUse
                                | hivecore_runtime_core::StopReason::StopSequence => {
                                    AcpStopReason::EndTurn
                                }
                                hivecore_runtime_core::StopReason::MaxTokens => {
                                    AcpStopReason::MaxTokens
                                }
                                hivecore_runtime_core::StopReason::Refusal => {
                                    AcpStopReason::Refusal
                                }
                                hivecore_runtime_core::StopReason::Error => AcpStopReason::Refusal,
                            },
                            Err(e) => {
                                tracing::error!(error = %e, "prompt turn failed");
                                AcpStopReason::Refusal
                            }
                        };
                        let _ = responder.respond(PromptResponse::new(acp_stop));
                        Ok(())
                    })?;
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_dispatch(
            async move |message: Dispatch, cx: ConnectionTo<Client>| {
                message.respond_with_error(
                    agent_client_protocol::util::internal_error("not implemented"),
                    cx,
                )
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
        .connect_to(ByteStreams::new(
            tokio::io::stdout().compat_write(),
            tokio::io::stdin().compat(),
        ))
        .await
        .map_err(|e| anyhow::anyhow!("acp connection: {e}"))
}

fn extract_text(blocks: &[ContentBlock]) -> String {
    let mut s = String::new();
    for b in blocks {
        if let ContentBlock::Text(t) = b {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&t.text);
        }
    }
    s
}

/// Adapter that owns a `ConnectionTo<Client>` and implements our internal
/// `NotificationSender` trait so the bridge sink can publish ACP
/// `session/update` notifications without depending on the SDK directly.
#[derive(Clone)]
struct ConnectionSender {
    cx: ConnectionTo<agent_client_protocol::Client>,
    session_id: SessionId,
}

impl ConnectionSender {
    fn new(cx: ConnectionTo<agent_client_protocol::Client>, session_id: SessionId) -> Self {
        Self { cx, session_id }
    }

    fn push(&self, update: SessionUpdate) {
        let notif = SessionNotification::new(self.session_id.clone(), update);
        if let Err(e) = self.cx.send_notification(notif) {
            tracing::warn!(error = %e, "failed to send session/update");
        }
    }
}

impl std::fmt::Debug for ConnectionSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionSender")
            .field("session_id", &self.session_id.0.as_ref())
            .finish()
    }
}

impl NotificationSender for ConnectionSender {
    fn send_text_chunk(&self, _session_id: &str, text: &str) {
        let chunk = ContentChunk::new(ContentBlock::from_text(text.to_string()));
        self.push(SessionUpdate::AgentMessageChunk(chunk));
    }

    fn send_thought_chunk(&self, _session_id: &str, text: &str) {
        let chunk = ContentChunk::new(ContentBlock::from_text(text.to_string()));
        self.push(SessionUpdate::AgentThoughtChunk(chunk));
    }

    fn send_tool_call(&self, _session_id: &str, tool_call_id: &str, name: &str, input: &Value) {
        let tc = ToolCall::new(ToolCallId::new(tool_call_id), name).raw_input(Some(input.clone()));
        self.push(SessionUpdate::ToolCall(tc));
    }

    fn send_tool_call_update(
        &self,
        _session_id: &str,
        tool_call_id: &str,
        is_error: bool,
        result: &Value,
    ) {
        let status = if is_error {
            ToolCallStatus::Failed
        } else {
            ToolCallStatus::Completed
        };
        let mut fields = ToolCallUpdateFields::default();
        fields.status = Some(status);
        fields.raw_output = Some(result.clone());
        let upd = ToolCallUpdate::new(ToolCallId::new(tool_call_id), fields);
        self.push(SessionUpdate::ToolCallUpdate(upd));
    }
}

#[async_trait]
impl PermissionRequester for ConnectionSender {
    #[allow(unused_variables)] // async-trait macro wraps args; false-positive
    async fn request_permission(
        &self,
        _session_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        raw_input: &Value,
        reason: Option<&str>,
    ) -> AcpPermissionDecision {
        // Build the ToolCallUpdate that the editor renders while pending.
        let mut fields = ToolCallUpdateFields::default();
        fields.status = Some(ToolCallStatus::Pending);
        fields.title = Some(reason.unwrap_or(tool_name).to_string());
        fields.raw_input = Some(raw_input.clone());
        let tool_call = ToolCallUpdate::new(ToolCallId::new(tool_call_id), fields);

        // Standard 4-option ACP vocabulary (`PermissionOptionKind` —
        // `agent-client-protocol-schema/client.rs:671-680`).
        let options = vec![
            PermissionOption::new(
                PermissionOptionId::new("allow_once"),
                "Allow once",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("allow_session"),
                "Allow for session",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject_once"),
                "Reject",
                PermissionOptionKind::RejectOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject_always"),
                "Reject and remember",
                PermissionOptionKind::RejectAlways,
            ),
        ];

        let req = RequestPermissionRequest::new(self.session_id.clone(), tool_call, options);
        let resp = match self.cx.send_request(req).block_task().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "session/request_permission failed; treating as Cancelled");
                return AcpPermissionDecision::Cancelled;
            }
        };

        match resp.outcome {
            RequestPermissionOutcome::Cancelled => AcpPermissionDecision::Cancelled,
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome { option_id, .. }) => {
                match option_id.0.as_ref() {
                    "allow_once" => AcpPermissionDecision::AllowOnce,
                    "allow_session" => AcpPermissionDecision::AllowAlways,
                    "reject_once" => AcpPermissionDecision::RejectOnce,
                    "reject_always" => AcpPermissionDecision::RejectAlways,
                    other => {
                        tracing::warn!(option_id = %other, "unknown PermissionOptionId; treating as RejectOnce");
                        AcpPermissionDecision::RejectOnce
                    }
                }
            }
            // SDK enum is `#[non_exhaustive]`; future variants treated as Cancelled
            // (safe-default — never auto-allow on unknown).
            _ => {
                tracing::warn!("unknown RequestPermissionOutcome variant; treating as Cancelled");
                AcpPermissionDecision::Cancelled
            }
        }
    }
}

trait ContentBlockExt {
    fn from_text(text: String) -> ContentBlock;
}

impl ContentBlockExt for ContentBlock {
    fn from_text(text: String) -> ContentBlock {
        use agent_client_protocol::schema::TextContent;
        ContentBlock::Text(TextContent::new(text))
    }
}

/// Builder helper for `ToolCall::raw_input` — the SDK exposes it as a
/// public field but no fluent setter, so we add one. Clippy's
/// `dead_code` heuristic doesn't follow trait dispatch on a public
/// SDK type, hence the allow.
#[allow(dead_code)]
trait ToolCallRawInputExt {
    fn raw_input(self, v: Option<Value>) -> Self;
}

impl ToolCallRawInputExt for ToolCall {
    fn raw_input(mut self, v: Option<Value>) -> Self {
        self.raw_input = v;
        self
    }
}
