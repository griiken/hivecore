//! `/v1/chat/completions` streaming wiring. Returns a `Stream<ModelChunk>`.

use async_stream::try_stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use hivecore_runtime_core::{AbortSignal, ModelChunk, RuntimeError, RuntimeResult};

use crate::client::OpenAiClient;
use crate::convert::{build_request, StreamAggregator};
use crate::error::OpenAiError;
use crate::sse::{SseDecoder, SseEvent};
use crate::wire::{ApiErrorEnvelope, ChatStreamEvent};

impl OpenAiClient {
    pub fn stream_chat(
        &self,
        req: hivecore_runtime_core::ModelRequest,
        signal: AbortSignal,
    ) -> impl Stream<Item = RuntimeResult<ModelChunk>> + Send + 'static {
        let http = self.http.clone();
        let url = self.endpoint("/chat/completions");

        try_stream! {
            let body = build_request(&req, true).map_err(map_err)?;
            let resp = http
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| map_err(OpenAiError::from(e)))?;

            let status = resp.status();
            if !status.is_success() {
                let code = status.as_u16();
                let text = resp.text().await.unwrap_or_default();
                let message = serde_json::from_str::<ApiErrorEnvelope>(&text)
                    .map(|e| e.error.message)
                    .unwrap_or(text);
                Err(map_err(OpenAiError::Api {
                    status: code,
                    message,
                }))?;
                return;
            }

            let mut bytes = resp.bytes_stream();
            let mut decoder = SseDecoder::new();
            let mut agg = StreamAggregator::new();

            while let Some(next) = bytes.next().await {
                if signal.is_aborted() {
                    Err(RuntimeError::Aborted)?;
                }
                let chunk: Bytes = next.map_err(|e| map_err(OpenAiError::from(e)))?;
                for ev in decoder.feed(chunk) {
                    match ev {
                        SseEvent::Data(json) => {
                            let parsed: ChatStreamEvent = serde_json::from_str(&json)
                                .map_err(|e| map_err(OpenAiError::from(e)))?;
                            for out in agg.ingest(parsed)? {
                                yield out;
                            }
                        }
                        SseEvent::Done => {
                            if let Some(end) = agg.finalize() {
                                yield end;
                            }
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn map_err(e: OpenAiError) -> RuntimeError {
    match e {
        OpenAiError::Api { status, message } => {
            RuntimeError::ModelFailed(format!("openai {status}: {message}"))
        }
        other => RuntimeError::ModelFailed(other.to_string()),
    }
}
