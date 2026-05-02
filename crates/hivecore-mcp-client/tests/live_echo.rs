//! Live e2e against the official `@modelcontextprotocol/server-everything`
//! MCP test server, spawned via `npx`.
//!
//! Marked `#[ignore]` so CI without npx + network access doesn't fail. Run
//! manually:
//!
//!   cargo test -p hivecore-mcp-client --test live_echo -- --ignored --nocapture
//!
//! First run pulls the npm package (~10s); subsequent runs are cached by npx.

use hivecore_mcp_client::{call_raw_tool, list_raw_tools, McpClient, McpUserConfig};

const TEST_SERVER_NAME: &str = "everything";

fn test_config() -> McpUserConfig {
    let toml_src = format!(
        r#"
        [mcp_servers.{TEST_SERVER_NAME}]
        transport = "stdio"
        command = "npx"
        args = ["-y", "@modelcontextprotocol/server-everything"]
        startup_timeout_sec = 60
        tool_timeout_sec = 30
        "#
    );
    McpUserConfig::from_toml_str(&toml_src).expect("config parses")
}

fn skip_if_no_npx() -> bool {
    which::which("npx").is_err()
}

#[tokio::test]
#[ignore]
async fn discover_lists_tools_from_everything_server() {
    if skip_if_no_npx() {
        eprintln!("[skip] npx not on PATH");
        return;
    }
    let client = McpClient::new(test_config());
    let v = list_raw_tools(&client, TEST_SERVER_NAME)
        .await
        .expect("list_tools");
    let tools = v
        .get("tools")
        .and_then(|t| t.as_array())
        .expect("tools array");
    assert!(!tools.is_empty(), "expected ≥1 tool from echo server");
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.iter().any(|n| *n == "echo"),
        "expected `echo` tool in {names:?}"
    );
    eprintln!("[ok] discovered {} tools: {:?}", names.len(), names);
}

#[tokio::test]
#[ignore]
async fn echo_round_trips_arguments() {
    if skip_if_no_npx() {
        eprintln!("[skip] npx not on PATH");
        return;
    }
    let client = McpClient::new(test_config());
    let payload = "hivecore-mcp-rountrip-marker";
    let result = call_raw_tool(
        &client,
        TEST_SERVER_NAME,
        "echo",
        serde_json::json!({ "message": payload }),
    )
    .await
    .expect("call_tool");

    // Result shape per MCP spec: { content: [{ type, text }, ...], isError? }
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("content array");
    let joined: String = content
        .iter()
        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("");
    assert!(
        joined.contains(payload),
        "echo output `{joined}` did not contain payload `{payload}`"
    );
    eprintln!("[ok] echo round-trip passed (got: {joined})");
}

#[tokio::test]
#[ignore]
async fn cache_reuses_connection_across_calls() {
    if skip_if_no_npx() {
        eprintln!("[skip] npx not on PATH");
        return;
    }
    let client = McpClient::new(test_config());
    let _ = list_raw_tools(&client, TEST_SERVER_NAME)
        .await
        .expect("first list");
    let t0 = std::time::Instant::now();
    let _ = list_raw_tools(&client, TEST_SERVER_NAME)
        .await
        .expect("second list");
    let elapsed = t0.elapsed();
    // Second list should be <500ms since the child process is already connected.
    // Cold spawn typically takes 5-15s on first run.
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "second list_tools took {elapsed:?} — connection cache likely not hit"
    );
    eprintln!("[ok] cached call returned in {elapsed:?}");
}
