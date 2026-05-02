//! OpenAI Chat Completions adapter (spike).
//!
//! Targets `/v1/chat/completions` with streaming. Works against OpenAI proper
//! and any OpenAI-compatible endpoint (groq, openrouter, vllm, ollama, …) by
//! flipping the base URL — the wire shape is identical.
//!
//! Module split mirrors the Zed `crates/open_ai/` convention: one file per
//! API endpoint family, plus `wire`/`convert`/`client` separation.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod adapter;
pub mod client;
pub mod completion;
pub mod convert;
pub mod error;
pub mod sse;
pub mod wire;

pub use adapter::OpenAiAdapter;
pub use client::{OpenAiClient, OpenAiConfig};
pub use error::OpenAiError;
