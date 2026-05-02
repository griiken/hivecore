use super::*;
use tempfile::TempDir;

fn root() -> (TempDir, WorkspaceRoot) {
    let dir = TempDir::new().unwrap();
    let root = WorkspaceRoot::new(dir.path()).unwrap();
    (dir, root)
}

#[test]
fn resolves_relative_path_under_root() {
    let (_d, root) = root();
    let p = root.resolve("foo.txt").unwrap();
    assert!(p.starts_with(root.path()));
}

#[test]
fn rejects_parent_dir_escape() {
    let (_d, root) = root();
    assert!(matches!(
        root.resolve("../outside"),
        Err(ToolError::PathEscape(_))
    ));
}

#[test]
fn rejects_absolute_path_outside_root() {
    let (_d, root) = root();
    let err = root.resolve("/etc/passwd").unwrap_err();
    assert!(matches!(err, ToolError::PathEscape(_)));
}

#[test]
fn allows_nested_subpath() {
    let (dir, root) = root();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let p = root.resolve("sub/x.txt").unwrap();
    assert!(p.starts_with(root.path()));
    assert!(p.ends_with("sub/x.txt"));
}
