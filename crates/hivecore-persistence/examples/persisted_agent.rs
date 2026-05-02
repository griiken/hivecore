//! Live demo: agent runs with sessions + audit sinks, then we replay the
//! session file off disk.
//!
//!     OPENAI_API_KEY=... cargo run --example persisted_agent

use std::env;
use std::sync::Arc;

use hivecore_agent_loop::{driver::user_text, AgentLoop, ToolRegistry};
use hivecore_builtin_tools::{default_set, WorkspaceRoot};
use hivecore_openai_adapter::{OpenAiAdapter, OpenAiClient, OpenAiConfig};
use hivecore_persistence::{AuditWriter, SessionHeader, SessionReader, SessionWriter, TenantId};
use hivecore_runtime_core::{
    AbortSignal, AgentMessage, ContentBlock, EventSink, FanOutSink, ModelAdapter, SessionId,
};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model_id = env::args().nth(1).unwrap_or_else(|| "gpt-5.4-nano".into());

    let workspace = TempDir::new()?;
    std::fs::write(
        workspace.path().join("README.md"),
        "# Project\n\nNo description.\n",
    )?;

    let log_dir = TempDir::new()?;
    let session_id = SessionId::new();
    let session_path = log_dir
        .path()
        .join(format!("{}.session.jsonl", session_id.0));
    let audit_path = log_dir.path().join(format!("{}.audit.jsonl", session_id.0));

    println!("session log: {}", session_path.display());
    println!("audit log:   {}", audit_path.display());

    let header = SessionHeader::new(session_id, &model_id, "You are a SWE assistant.");
    let session_writer: Arc<dyn EventSink> =
        Arc::new(SessionWriter::create(&session_path, header).await?);
    let audit_writer: Arc<dyn EventSink> =
        Arc::new(AuditWriter::create(&audit_path, session_id, TenantId::single_tenant()).await?);
    let sink: Arc<dyn EventSink> = Arc::new(FanOutSink::new(vec![session_writer, audit_writer]));

    let root = WorkspaceRoot::new(workspace.path())?;
    let tools = ToolRegistry::new(default_set(root));

    let client = OpenAiClient::new(OpenAiConfig::new(api_key))?;
    let adapter: Arc<dyn ModelAdapter> = Arc::new(OpenAiAdapter::new(client));

    let mut agent = AgentLoop::builder()
        .session_id(session_id)
        .system_prompt("You are a concise SWE assistant. Use tools.")
        .model(adapter, model_id)
        .tools(tools)
        .sink(sink)
        .max_iterations(8)
        .build()?;

    let task = "Read README.md, then edit it to add the line `Hivecore is great.` after the existing description. Show me the final contents.";
    let (_h, sig) = AbortSignal::new();
    let outcome = agent.run(user_text(task), sig).await?;

    println!(
        "\n--- live run done. stop={:?} appended={}",
        outcome.stop_reason, outcome.messages_appended
    );

    // Replay from disk.
    println!("\n--- replaying session from disk ---");
    let r = SessionReader::open(&session_path).await?;
    println!(
        "header: model={} created_at={}",
        r.header.model, r.header.created_at
    );
    println!("event_count: {}", r.event_count());
    let msgs = r.messages();
    println!("message_count: {}", msgs.len());
    for (i, m) in msgs.iter().enumerate() {
        let (role, snippet) = describe(m);
        println!("  [{}] {:>10}  {}", i, role, snippet);
    }

    // Audit log byte count + line count.
    let audit_text = tokio::fs::read_to_string(&audit_path).await?;
    println!(
        "\n--- audit log: {} lines / {} bytes ---",
        audit_text.lines().count(),
        audit_text.len()
    );

    Ok(())
}

fn describe(m: &AgentMessage) -> (&'static str, String) {
    match m {
        AgentMessage::User { content, .. } => ("user", first_text(content)),
        AgentMessage::Assistant { content, .. } => ("assistant", first_text(content)),
        AgentMessage::ToolResult {
            is_error, content, ..
        } => (
            if *is_error { "tool_err" } else { "tool" },
            first_text(content),
        ),
        AgentMessage::Custom { kind, .. } => ("custom", kind.clone()),
    }
}

fn first_text(content: &[ContentBlock]) -> String {
    for c in content {
        if let ContentBlock::Text { text } = c {
            let line = text.lines().next().unwrap_or(text).to_string();
            return if line.len() > 80 {
                format!("{}…", &line[..80])
            } else {
                line
            };
        }
        if let ContentBlock::ToolUse { name, .. } = c {
            return format!("[tool_use {}]", name);
        }
    }
    String::new()
}
