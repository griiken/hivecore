use hivecore_runtime_core::{AbortSignal, ToolCallId, ToolInvocation, UpdateSink};
use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn writes_file_and_creates_dir() {
    let dir = TempDir::new().unwrap();
    let tool = WriteTool::new(WorkspaceRoot::new(dir.path()).unwrap());
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
    let tool = WriteTool::new(WorkspaceRoot::new(dir.path()).unwrap());
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
