//! HTTP client. Holds the base URL, key, and a shared `reqwest::Client`.
//! The `completion` module borrows it for actual streaming.

use std::time::Duration;

use reqwest::{header, Client};

use crate::error::OpenAiError;

#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub api_key: String,
    /// Defaults to `https://api.openai.com/v1`. Override for compatible
    /// providers (groq, openrouter, vllm, ollama, …).
    pub base_url: String,
    pub timeout: Duration,
    /// Optional `OpenAI-Organization` / `OpenAI-Project` headers.
    pub organization: Option<String>,
    pub project: Option<String>,
}

impl OpenAiConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".into(),
            timeout: Duration::from_secs(120),
            organization: None,
            project: None,
        }
    }

    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base_url = base.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    pub(crate) http: Client,
    pub(crate) config: OpenAiConfig,
}

impl OpenAiClient {
    pub fn new(config: OpenAiConfig) -> Result<Self, OpenAiError> {
        let mut headers = header::HeaderMap::new();
        let auth = format!("Bearer {}", config.api_key);
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&auth)
                .map_err(|e| OpenAiError::Unsupported(format!("invalid api key: {e}")))?,
        );
        if let Some(org) = &config.organization {
            headers.insert(
                "OpenAI-Organization",
                header::HeaderValue::from_str(org)
                    .map_err(|e| OpenAiError::Unsupported(format!("invalid org: {e}")))?,
            );
        }
        if let Some(proj) = &config.project {
            headers.insert(
                "OpenAI-Project",
                header::HeaderValue::from_str(proj)
                    .map_err(|e| OpenAiError::Unsupported(format!("invalid project: {e}")))?,
            );
        }

        let http = Client::builder()
            .timeout(config.timeout)
            .default_headers(headers)
            .build()?;

        Ok(Self { http, config })
    }

    pub(crate) fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }
}
