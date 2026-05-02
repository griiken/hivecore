//! `ModelAdapter` impl that delegates to `OpenAiClient::stream_chat`.

use async_trait::async_trait;
use futures::StreamExt;
use hivecore_runtime_core::{AbortSignal, ModelAdapter, ModelRequest, ModelStream, RuntimeResult};

use crate::client::OpenAiClient;

#[derive(Debug, Clone)]
pub struct OpenAiAdapter {
    id: String,
    client: OpenAiClient,
}

impl OpenAiAdapter {
    pub fn new(client: OpenAiClient) -> Self {
        Self {
            id: "openai".into(),
            client,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
}

#[async_trait]
impl ModelAdapter for OpenAiAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    async fn complete(&self, req: ModelRequest, signal: AbortSignal) -> RuntimeResult<ModelStream> {
        let stream = self.client.stream_chat(req, signal).boxed();
        Ok(stream)
    }
}
