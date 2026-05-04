use std::sync::Arc;

use hivecore_execution_env::LocalEnv;
use hivecore_runtime_core::{AbortSignal, ExecutionEnv, ToolCallId, ToolInvocation, UpdateSink};
use tempfile::TempDir;

use super::*;

fn invoke(input: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId("c".into()),
        name: "bash".into(),
        input,
    }
}

fn env_for(dir: &TempDir) -> Arc<dyn ExecutionEnv> {
    Arc::new(LocalEnv::new(dir.path().to_path_buf()))
}

#[tokio::test]
async fn runs_echo_and_captures_stdout() {
    let dir = TempDir::new().unwrap();
    let tool = BashTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            invoke(serde_json::json!({"command":"echo hello"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(!out.is_error);
    let text = match &out.content[0] {
        hivecore_runtime_core::ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert!(text.contains("hello"));
}

#[tokio::test]
async fn nonzero_exit_marks_is_error() {
    let dir = TempDir::new().unwrap();
    let tool = BashTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            invoke(serde_json::json!({"command":"exit 7"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(out.is_error);
    let exit = out.details.unwrap()["exit_code"].as_i64().unwrap();
    assert_eq!(exit, 7);
}

#[tokio::test]
async fn timeout_kills_long_running_command() {
    let dir = TempDir::new().unwrap();
    let tool = BashTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            invoke(serde_json::json!({"command":"sleep 5","timeout_ms":200})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("timeout"));
}
