//! Live smoke test against OpenAI. Run with:
//!     OPENAI_API_KEY=... cargo run --example smoke -- <model> "<prompt>"
//!
//! Streams the assistant reply to stdout and prints the usage summary.

use std::env;

use futures::StreamExt;
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_runtime_core::{
    AbortSignal, AgentMessage, ContentBlock, MessageId, ModelAdapter, ModelChunk, ModelRequest,
    ThinkingLevel,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let mut args = env::args().skip(1);
    let model = args.next().unwrap_or_else(|| "gpt-5.4-nano".into());
    let prompt = args
        .next()
        .unwrap_or_else(|| "Say hello in five words.".into());

    let client = OpenAiClient::new(OpenAiConfig::new(api_key))?;
    let adapter = OpenAiAdapter::new(client);

    let req = ModelRequest {
        model,
        system: "You are a concise assistant.".into(),
        messages: vec![AgentMessage::User {
            id: MessageId("u1".into()),
            content: vec![ContentBlock::Text { text: prompt }],
        }],
        tools: vec![],
        thinking: ThinkingLevel::Off,
        max_tokens: Some(128),
    };

    let (_handle, signal) = AbortSignal::new();
    let mut stream = adapter.complete(req, signal).await?;

    while let Some(chunk) = stream.next().await {
        match chunk? {
            ModelChunk::MessageStart { id } => eprintln!("[start id={}]", id.0),
            ModelChunk::ContentDelta { delta, .. } => match delta {
                ContentBlock::Text { text } => {
                    print!("{text}");
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    eprintln!("\n[tool_use {name} {input}]");
                }
                _ => {}
            },
            ModelChunk::MessageEnd {
                stop_reason, usage, ..
            } => {
                eprintln!(
                    "\n[end stop={stop_reason:?} in={} out={} cache_read={}]",
                    usage.input, usage.output, usage.cache_read
                );
            }
        }
    }
    Ok(())
}
