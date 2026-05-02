//! Live e2e for `browser_wait` predicate variants (v0.2 fix).
//!
//! Drives all variants that previously fell through to sleep-by-timeout:
//!   - SelectorVisible / SelectorHidden
//!   - UrlContains
//!   - TextVisible / TextHidden
//!
//! Run:
//!   cargo run --example wait -p hivecore-browser-harness

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
        id: ToolCallId(format!("wait-{}", tool.name())),
        name: tool.name().to_string(),
        input,
    }
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info")
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
    let wait = by_name("browser_wait");

    sess.execute(
        invoke(sess, json!({ "op": "open", "name": "wait-test" })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;

    // Page that adds a `#late` element after 200 ms via setTimeout.
    let html = r#"<html><body>
        <h1>Wait Demo</h1>
        <p id="now">visible-now</p>
        <script>
            setTimeout(() => {
                const d = document.createElement('div');
                d.id = 'late';
                d.textContent = 'late-arrival';
                document.body.appendChild(d);
            }, 200);
        </script>
    </body></html>"#;
    let url = format!("data:text/html;charset=utf-8,{}", urlencoding::encode(html));
    nav.execute(
        invoke(nav, json!({ "session": "wait-test", "url": url })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;
    println!("[ok] navigated");

    // ---- selector_visible (positive — element appears within 2s) ----
    let t0 = std::time::Instant::now();
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "selector_visible",
                    "selector": "#late",
                    "timeout_ms": 2000
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let elapsed = t0.elapsed().as_millis();
    println!(
        "[ok] selector_visible #late → {} in {}ms",
        text_of(&r).trim(),
        elapsed
    );
    anyhow::ensure!(!r.is_error, "selector_visible failed: {}", text_of(&r));
    anyhow::ensure!(
        elapsed >= 150,
        "fired too early — should wait ~200ms for setTimeout"
    );
    anyhow::ensure!(
        elapsed < 2000,
        "fired too late — should resolve before timeout"
    );

    // ---- selector_visible (negative — bogus selector, must time out) ----
    let t0 = std::time::Instant::now();
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "selector_visible",
                    "selector": "#never-exists",
                    "timeout_ms": 600
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let elapsed = t0.elapsed().as_millis();
    let msg = text_of(&r);
    println!(
        "[ok] selector_visible #never-exists → error in {}ms",
        elapsed
    );
    anyhow::ensure!(r.is_error, "expected timeout, got success");
    anyhow::ensure!(msg.contains("wait timeout"), "wrong error: {msg}");
    anyhow::ensure!(
        elapsed >= 500,
        "exited too early — should wait near timeout_ms"
    );
    anyhow::ensure!(
        elapsed < 1500,
        "exited too late — should respect timeout_ms"
    );

    // ---- text_visible (positive) ----
    let t0 = std::time::Instant::now();
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "text_visible",
                    "substring": "late-arrival",
                    "timeout_ms": 2000
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!(
        "[ok] text_visible 'late-arrival' → {} in {}ms",
        text_of(&r).trim(),
        t0.elapsed().as_millis()
    );
    anyhow::ensure!(!r.is_error, "text_visible failed: {}", text_of(&r));

    // ---- text_hidden (substring not in DOM, returns instantly) ----
    let t0 = std::time::Instant::now();
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "text_hidden",
                    "substring": "definitely-not-here",
                    "timeout_ms": 2000
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    let elapsed = t0.elapsed().as_millis();
    println!(
        "[ok] text_hidden 'definitely-not-here' → ok in {}ms",
        elapsed
    );
    anyhow::ensure!(!r.is_error, "text_hidden failed");
    anyhow::ensure!(
        elapsed < 500,
        "should resolve quickly when text already absent"
    );

    // ---- url_contains ----
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "url_contains",
                    "substring": "data:",
                    "timeout_ms": 1000
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!("[ok] url_contains 'data:' → {}", text_of(&r).trim());
    anyhow::ensure!(!r.is_error, "url_contains failed");

    // ---- url_matches (regex) ----
    let r = wait
        .execute(
            invoke(
                wait,
                json!({
                    "session": "wait-test",
                    "condition": "url_matches",
                    "pattern": r"^data:text/html.*Wait%20Demo",
                    "timeout_ms": 1000
                }),
            ),
            AbortSignal::new().1,
            UpdateSink::noop(),
        )
        .await?;
    println!("[ok] url_matches regex → {}", text_of(&r).trim());
    anyhow::ensure!(!r.is_error, "url_matches failed");

    sess.execute(
        invoke(sess, json!({ "op": "close", "name": "wait-test" })),
        AbortSignal::new().1,
        UpdateSink::noop(),
    )
    .await?;

    println!("\nWAIT TEST PASS");
    Ok(())
}
