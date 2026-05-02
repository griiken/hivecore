//! Live e2e smoke test for the v0.1 browser harness.
//!
//! Spawns Chrome via `ChromeProvider`, navigates to a data: URL with a
//! known DOM, runs `browser_session(open)` → `browser_navigate` →
//! `browser_snapshot` → `browser_assert` → `browser_screenshot`, prints
//! results. No agent loop, no model — just the tool surface invoked
//! directly.
//!
//! Run with:
//!   cargo run --example smoke -p hivecore-browser-harness

use std::sync::Arc;

use hivecore_browser_core::{BrowserProvider, TenantId};
use hivecore_browser_harness::{default_tools, BrowserHarness};
use hivecore_browser_runtime::{ChromeConfig, ChromeProvider};
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, Tool, ToolCallId, ToolInvocation, UpdateSink,
};
use serde_json::json;

fn invoke(tool: &dyn Tool, input: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId(format!("smoke-{}", tool.name())),
        name: tool.name().to_string(),
        input,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info,hivecore_browser_runtime=debug")
        .init();

    let provider: Arc<dyn BrowserProvider> = Arc::new(ChromeProvider::new(ChromeConfig::default()));
    let harness = BrowserHarness::new(provider, TenantId::default_tenant());
    let tools = default_tools(harness);
    println!("[ok] {} tools registered", tools.len());

    let sess = tools
        .iter()
        .find(|t| t.name() == "browser_session")
        .unwrap();
    let nav = tools
        .iter()
        .find(|t| t.name() == "browser_navigate")
        .unwrap();
    let snap_t = tools
        .iter()
        .find(|t| t.name() == "browser_snapshot")
        .unwrap();
    let assert_t = tools.iter().find(|t| t.name() == "browser_assert").unwrap();
    let shot = tools
        .iter()
        .find(|t| t.name() == "browser_screenshot")
        .unwrap();

    // open
    let r = sess
        .execute(
            invoke(sess.as_ref(), json!({ "op": "open", "name": "smoke" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!("[ok] open: {}", text_of(&r));
    anyhow::ensure!(!r.is_error, "open failed");

    // navigate
    let r = nav
        .execute(
            invoke(
                nav.as_ref(),
                json!({
                    "session": "smoke",
                    "url": "data:text/html,<html><body><h1>hello</h1><button>Submit</button><input type=text></body></html>"
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!("[ok] navigate: {}", text_of(&r));
    anyhow::ensure!(!r.is_error, "navigate failed");

    // snapshot
    let r = snap_t
        .execute(
            invoke(snap_t.as_ref(), json!({ "session": "smoke" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let details = r.details.as_ref().unwrap();
    let elements = details.get("elements").unwrap().as_array().unwrap();
    println!("[ok] snapshot: {} actionable elements", elements.len());
    let roles: Vec<_> = elements
        .iter()
        .filter_map(|e| e.get("role").and_then(|v| v.as_str()))
        .collect();
    println!("[ok] roles: {roles:?}");
    anyhow::ensure!(roles.contains(&"button"), "no button found");
    anyhow::ensure!(roles.contains(&"textbox"), "no textbox found");
    // Per validation report fix #5: heading + img are NOT actionable.
    // The h1 is present in DOM but should not appear in actionable list.
    anyhow::ensure!(
        !roles.contains(&"heading"),
        "heading should NOT be in actionable list (validation fix #5)"
    );

    // assert
    let r = assert_t
        .execute(
            invoke(
                assert_t.as_ref(),
                json!({
                    "session": "smoke",
                    "predicate": "url_contains",
                    "substring": "data:"
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let result = r
        .details
        .as_ref()
        .and_then(|d| d.get("result"))
        .and_then(|v| v.as_bool());
    println!("[ok] assert url_contains 'data:' → {result:?}");
    anyhow::ensure!(result == Some(true), "assert failed");

    // screenshot
    let r = shot
        .execute(
            invoke(shot.as_ref(), json!({ "session": "smoke" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let bytes = r
        .details
        .as_ref()
        .and_then(|d| d.get("bytes"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    println!("[ok] screenshot: {bytes} bytes");
    anyhow::ensure!(bytes > 0, "empty screenshot");

    // close
    let r = sess
        .execute(
            invoke(sess.as_ref(), json!({ "op": "close", "name": "smoke" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!("[ok] close: {}", text_of(&r));

    println!("\nSMOKE PASS");
    Ok(())
}

fn text_of(r: &hivecore_runtime_core::ToolOutcome) -> String {
    r.content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
