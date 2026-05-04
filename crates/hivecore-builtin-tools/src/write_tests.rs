use std::sync::Arc;

use hivecore_execution_env::LocalEnv;
use hivecore_runtime_core::{AbortSignal, ExecutionEnv, ToolCallId, ToolInvocation, UpdateSink};
use tempfile::TempDir;

use super::*;

fn env_for(dir: &TempDir) -> Arc<dyn ExecutionEnv> {
    Arc::new(LocalEnv::new(dir.path().to_path_buf()))
}

#[tokio::test]
async fn writes_file_and_creates_dir() {
    let dir = TempDir::new().unwrap();
    let tool = WriteTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            ToolInvocation {
                id: ToolCallId("c".into()),
                name: "write_file".into(),
                input: serde_json::json!({"path":"deep/nested/x.txt","content":"hi"}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(!out.is_error);
    let written = std::fs::read_to_string(dir.path().join("deep/nested/x.txt")).unwrap();
    assert_eq!(written, "hi");
}

#[tokio::test]
async fn rejects_escape() {
    let dir = TempDir::new().unwrap();
    let tool = WriteTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            ToolInvocation {
                id: ToolCallId("c".into()),
                name: "write_file".into(),
                input: serde_json::json!({"path":"../naughty","content":"x"}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("escapes workspace"));
}
