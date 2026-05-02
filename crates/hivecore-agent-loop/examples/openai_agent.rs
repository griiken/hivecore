//! End-to-end smoke: gpt-5.4-nano + agent loop + a single arithmetic tool.
//! Run with:
//!     OPENAI_API_KEY=... cargo run --example openai_agent

use std::env;
use std::sync::Arc;

use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry, VecSink};
use async_trait::async_trait;
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_runtime_core::{
    AbortSignal, AgentEvent, ContentBlock, ModelAdapter, RuntimeResult, Tool, ToolInvocation,
    ToolOutcome, UpdateSink,
};

struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &str {
        "add"
    }
    fn description(&self) -> &str {
        "Add two integers and return their sum."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "a": {"type": "integer"},
                "b": {"type": "integer"}
            },
            "required": ["a", "b"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let a = invocation
            .input
            .get("a")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let b = invocation
            .input
            .get("b")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let sum = a + b;
        Ok(ToolOutcome::ok(vec![ContentBlock::Text {
            text: sum.to_string(),
        }]))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model_id = env::args().nth(1).unwrap_or_else(|| "gpt-5.4-nano".into());
    let prompt = env::args()
        .nth(2)
        .unwrap_or_else(|| "What is 17 + 25? Use the add tool.".into());

    let client = OpenAiClient::new(OpenAiConfig::new(api_key))?;
    let adapter: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));

    let tools = ToolRegistry::new(vec![Arc::new(AddTool) as Arc<dyn Tool>]);
    let sink = Arc::new(VecSink::new());

    let mut agent = AgentLoop::builder()
        .system_prompt("You are a concise assistant. Use tools when asked.")
        .model(adapter, model_id)
        .tools(tools)
        .sink(sink.clone())
        .max_iterations(8)
        .build()?;

    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text(prompt), sig).await?;

    println!("\n--- run complete ---");
    println!("stop_reason  = {:?}", outcome.stop_reason);
    println!("appended_msg = {}", outcome.messages_appended);

    let events = sink.snapshot();
    println!("events       = {}", events.len());
    for ev in &events {
        match ev {
            AgentEvent::TurnStart { turn_id, .. } => println!("  TurnStart {}", turn_id.0),
            AgentEvent::ToolExecStart { name, input, .. } => {
                println!("  ToolExecStart {name} {input}")
            }
            AgentEvent::ToolExecEnd { is_error, .. } => {
                println!("  ToolExecEnd error={is_error}")
            }
            AgentEvent::TurnEnd { stop_reason, .. } => {
                println!("  TurnEnd stop={stop_reason:?}")
            }
            _ => {}
        }
    }

    // Print the final assistant text.
    if let Some(hivecore_runtime_core::AgentMessage::Assistant { content, .. }) =
        agent.state().messages.last()
    {
        for c in content {
            if let ContentBlock::Text { text } = c {
                println!("\nAssistant: {text}");
            }
        }
    }

    Ok(())
}
