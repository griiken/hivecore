//! End-to-end: load `examples/skills/`, plug skills into an agent loop, run
//! a prompt that lets gpt-5.4-nano pick the `word-count` skill.
//!
//!     OPENAI_API_KEY=... cargo run --example use_skill

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry};
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_runtime_core::{
    AbortSignal, AgentEvent, AgentMessage, ContentBlock, ModelAdapter, VecSink,
};
use hivecore_skills::{render::RenderCtx, SkillLoader, SkillRegistry};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model_id = env::args().nth(1).unwrap_or_else(|| "gpt-5.4-nano".into());

    // Sandbox workspace seeded with one file.
    let workspace = TempDir::new()?;
    std::fs::write(
        workspace.path().join("notes.txt"),
        "alpha beta gamma delta\nepsilon zeta\n",
    )?;
    println!("workspace: {}", workspace.path().display());

    // Load the bundled skills directory.
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/skills");
    let skills = SkillLoader::new().add_dir(&skills_dir).load()?;
    let registry = SkillRegistry::from_skills(skills)?;
    println!("loaded skills: {:?}", registry.names());

    let workdir = workspace.path().to_path_buf();
    let tools = registry.model_invokable_tools(move || RenderCtx {
        workdir: workdir.clone(),
        session_id: "demo-session".into(),
        ..Default::default()
    });
    let tools = ToolRegistry::new(tools);

    let client = OpenAiClient::new(OpenAiConfig::new(api_key))?;
    let adapter: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));
    let sink = Arc::new(VecSink::new());

    let mut agent = AgentLoop::builder()
        .system_prompt(
            "You are a concise assistant. When the user asks for word counts, \
             use the `word-count` skill — it pre-renders an answer string for \
             you. Do not run other tools.",
        )
        .model(adapter, model_id)
        .tools(tools)
        .sink(sink.clone())
        .max_iterations(8)
        .build()?;

    let task = "How many words are in notes.txt?";
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text(task), sig).await?;

    println!("\n--- run complete ---");
    println!("stop_reason  = {:?}", outcome.stop_reason);
    println!("appended_msg = {}", outcome.messages_appended);
    for ev in sink.snapshot() {
        if let AgentEvent::ToolExecStart { name, input, .. } = ev {
            println!("  → {name}({})", trunc(&input.to_string()));
        }
    }
    if let Some(AgentMessage::Assistant { content, .. }) = agent.state().messages.last() {
        for c in content {
            if let ContentBlock::Text { text } = c {
                println!("\nAssistant: {text}");
            }
        }
    }
    Ok(())
}

fn trunc(s: &str) -> String {
    if s.len() > 120 {
        format!("{}…", &s[..120])
    } else {
        s.to_string()
    }
}
