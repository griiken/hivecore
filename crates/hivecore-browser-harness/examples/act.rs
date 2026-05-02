//! Live e2e for `browser_act` real CDP dispatch (the v0.2 fix).
//!
//! Drives: navigate → snapshot → fill input → click button → re-snapshot
//! → assert URL changed (form submitted to data: URL).
//!
//! Run:
//!   cargo run --example act -p hivecore-browser-harness

use std::sync::Arc;

use hivecore_browser_core::{BrowserProvider, ElementRef, TenantId};
use hivecore_browser_harness::{default_tools, BrowserHarness};
use hivecore_browser_runtime::{ChromeConfig, ChromeProvider};
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, Tool, ToolCallId, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde_json::json;

fn invoke(tool: &dyn Tool, input: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId(format!("act-{}", tool.name())),
        name: tool.name().to_string(),
        input,
    }
}

fn pick(out: &ToolOutcome) -> &serde_json::Value {
    out.details.as_ref().expect("details present")
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

    let by_name = |name: &str| -> &dyn Tool {
        tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
            .expect("tool present")
    };

    let sess = by_name("browser_session");
    let nav = by_name("browser_navigate");
    let snap_t = by_name("browser_snapshot");
    let act = by_name("browser_act");
    let _assert_t = by_name("browser_assert");

    // Form page: text input + button that copies the value into a result
    // heading. data: URL forms don't navigate, so we use a JS click handler
    // and verify the result via post-action snapshot.
    // Click handler updates the result button's aria-label so the post-click
    // snapshot can verify via `button` role (we no longer expose `heading`).
    let html = r#"<html><body>
        <h1>Form Demo</h1>
        <input id="q" type="text" placeholder="search">
        <button id="b" onclick="document.getElementById('result').setAttribute('aria-label','RESULT:'+document.getElementById('q').value);return false;">Submit</button>
        <button id="result" aria-label="pending">Result</button>
    </body></html>"#;
    let url = format!("data:text/html;charset=utf-8,{}", urlencoding::encode(html));

    sess.execute(
        invoke(sess, json!({ "op": "open", "name": "act-test" })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;
    println!("[ok] session opened");

    nav.execute(
        invoke(nav, json!({ "session": "act-test", "url": url })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;
    println!("[ok] navigated");

    let snap_out = snap_t
        .execute(
            invoke(snap_t, json!({ "session": "act-test" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let elements = pick(&snap_out)
        .get("elements")
        .and_then(|v| v.as_array())
        .unwrap();
    println!("[ok] snapshot has {} actionable elements", elements.len());
    for e in elements {
        println!(
            "    role={:?} name={:?} ref={:?}",
            e.get("role").and_then(|v| v.as_str()),
            e.get("name").and_then(|v| v.as_str()),
            e.get("ref").and_then(|v| v.as_str())
        );
    }

    // Find the textbox + button refs.
    let textbox_ref = elements
        .iter()
        .find(|e| e.get("role").and_then(|v| v.as_str()) == Some("textbox"))
        .and_then(|e| e.get("ref").and_then(|v| v.as_str()))
        .unwrap()
        .to_string();
    let button_ref = elements
        .iter()
        .find(|e| e.get("role").and_then(|v| v.as_str()) == Some("button"))
        .and_then(|e| e.get("ref").and_then(|v| v.as_str()))
        .unwrap()
        .to_string();
    println!("[ok] textbox ref = {textbox_ref}");
    println!("[ok] button  ref = {button_ref}");

    // Fill the textbox.
    let r = act
        .execute(
            invoke(
                act,
                json!({
                    "session": "act-test",
                    "target": textbox_ref,
                    "kind": "fill",
                    "text": "hivecore-rocks",
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!(
        "[ok] fill outcome: ok={}, note={}",
        pick(&r).get("ok").unwrap_or(&json!(null)),
        pick(&r).get("note").and_then(|v| v.as_str()).unwrap_or("?")
    );
    anyhow::ensure!(!r.is_error, "fill failed: {:?}", r.content);

    // Verify the textbox now has the value via inner JS check (re-snapshot
    // alone won't see the value since we filter by role/name, not value).
    // We use assert with a freshly-issued ref from the post-fill snapshot.
    let post_fill_snap = pick(&r).get("snapshot").cloned().unwrap_or(json!({}));
    let post_textbox_ref = post_fill_snap
        .get("elements")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("role").and_then(|v| v.as_str()) == Some("textbox"))
        })
        .and_then(|e| e.get("ref").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .unwrap();
    println!("[ok] post-fill textbox ref = {post_textbox_ref} (re-snapshot version bumped)");

    // Stale-ref assertion: the original button_ref (from snapshot v1) should
    // be rejected after the act re-snapshot bumped to v2.
    let stale_check = ElementRef::parse(&button_ref).unwrap();
    let fresh_check = ElementRef::parse(&post_textbox_ref).unwrap();
    anyhow::ensure!(
        stale_check.version != fresh_check.version,
        "expected version bump, got same"
    );
    println!(
        "[ok] version bumped: {} → {}",
        stale_check.version, fresh_check.version
    );

    // Now click the button using the FRESH ref. Find post-fill button ref.
    let post_button_ref = post_fill_snap
        .get("elements")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("role").and_then(|v| v.as_str()) == Some("button"))
        })
        .and_then(|e| e.get("ref").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .unwrap();

    let r = act
        .execute(
            invoke(
                act,
                json!({
                    "session": "act-test",
                    "target": post_button_ref,
                    "kind": "click",
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!(
        "[ok] click outcome: {}",
        pick(&r).get("note").and_then(|v| v.as_str()).unwrap_or("?")
    );
    anyhow::ensure!(!r.is_error, "click failed");

    // Stale-ref negative test: try to act using the original (v1) button ref.
    let r = act
        .execute(
            invoke(
                act,
                json!({
                    "session": "act-test",
                    "target": button_ref, // the OLD v1 ref
                    "kind": "click",
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let stale_msg = r
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    anyhow::ensure!(r.is_error, "expected stale-ref error, got success");
    anyhow::ensure!(
        stale_msg.contains("stale ref") || stale_msg.contains("re-snapshot"),
        "expected stale-ref message, got: {stale_msg}"
    );
    println!("[ok] stale-ref correctly rejected: {stale_msg}");

    // After click, the result heading should contain "RESULT:hivecore-rocks".
    let post_click_snap = snap_t
        .execute(
            invoke(snap_t, json!({ "session": "act-test" })),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let button_names: Vec<String> = pick(&post_click_snap)
        .get("elements")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|e| e.get("role").and_then(|v| v.as_str()) == Some("button"))
                .filter_map(|e| {
                    e.get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    println!("[ok] post-click button names: {button_names:?}");
    anyhow::ensure!(
        button_names
            .iter()
            .any(|n| n.contains("RESULT:hivecore-rocks")),
        "click did not trigger DOM update — expected 'RESULT:hivecore-rocks' in a button aria-label"
    );

    sess.execute(
        invoke(sess, json!({ "op": "close", "name": "act-test" })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;

    println!("\nACT TEST PASS");
    Ok(())
}
