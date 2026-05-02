//! Live SWE demo: agent reads a file, edits it, runs a shell command to
//! verify. Run with:
//!     OPENAI_API_KEY=... cargo run --example swe_agent

use std::env;
use std::sync::Arc;

use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry, VecSink};
use hivecore_builtin_tools::{default_set, WorkspaceRoot};
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_runtime_core::{AbortSignal, AgentEvent, ContentBlock, ModelAdapter};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model_id = env::args().nth(1).unwrap_or_else(|| "gpt-5.4-nano".into());

    // Sandbox: temp dir seeded with one file.
    let dir = TempDir::new()?;
    std::fs::write(
        dir.path().join("hello.py"),
        "def greet(name):\n    return f\"Hello {name}\"\n\nprint(greet('world'))\n",
    )?;
    println!("workspace: {}", dir.path().display());

    let root = WorkspaceRoot::new(dir.path())?;
    let tools = ToolRegistry::new(default_set(root));

    let client = OpenAiClient::new(OpenAiConfig::new(api_key))?;
    let adapter: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));
    let sink = Arc::new(VecSink::new());

    let mut agent = AgentLoop::builder()
        .system_prompt(
            "You are a software engineer working in a sandbox workspace. \
             Use the read_file, edit_file, write_file, bash, and grep tools \
             to complete the user's request. Be concise.",
        )
        .model(adapter, model_id)
        .tools(tools)
        .sink(sink.clone())
        .max_iterations(16)
        .build()?;

    let task = "Read hello.py. Change the greeting from \"Hello\" to \"Hi\". \
                Then run `python3 hello.py` to verify. Report what it printed.";
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text(task), sig).await?;

    println!("\n--- run complete ---");
    println!("stop_reason  = {:?}", outcome.stop_reason);
    println!("appended_msg = {}", outcome.messages_appended);

    for ev in sink.snapshot() {
        match ev {
            AgentEvent::ToolExecStart { name, input, .. } => {
                let preview = input.to_string();
                let preview = if preview.len() > 80 {
                    format!("{}…", &preview[..80])
                } else {
                    preview
                };
                println!("  → {name} {preview}");
            }
            AgentEvent::ToolExecEnd { is_error, .. } => {
                if is_error {
                    println!("    [error]");
                }
            }
            AgentEvent::TurnEnd { stop_reason, .. } => {
                println!("  turn-end {stop_reason:?}");
            }
            _ => {}
        }
    }

    if let Some(hivecore_runtime_core::AgentMessage::Assistant { content, .. }) =
        agent.state().messages.last()
    {
        println!();
        for c in content {
            if let ContentBlock::Text { text } = c {
                println!("Assistant: {text}");
            }
        }
    }

    println!("\nFinal hello.py:");
    println!("{}", std::fs::read_to_string(dir.path().join("hello.py"))?);

    Ok(())
}
