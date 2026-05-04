use super::*;
use hivecore_runtime_core::ExecutionEnv;
use tempfile::tempdir;

#[tokio::test]
async fn write_then_read_round_trips() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let p = dir.path().join("hello.txt");
    env.write_file(&p, b"hi").await.unwrap();
    let bytes = env.read_file(&p).await.unwrap();
    assert_eq!(bytes, b"hi");
    let text = env.read_text_file(&p).await.unwrap();
    assert_eq!(text, "hi");
}

#[tokio::test]
async fn write_creates_parent_dirs() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let p = dir.path().join("a/b/c/file.txt");
    env.write_file(&p, b"xyz").await.unwrap();
    assert!(env.path_exists(&p).await.unwrap());
}

#[tokio::test]
async fn list_dir_returns_sorted_children() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    env.write_file(&dir.path().join("b"), b"").await.unwrap();
    env.write_file(&dir.path().join("a"), b"").await.unwrap();
    env.write_file(&dir.path().join("c"), b"").await.unwrap();
    let names: Vec<_> = env
        .list_dir(dir.path())
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn stat_distinguishes_file_and_dir() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let f = dir.path().join("f");
    env.write_file(&f, b"x").await.unwrap();
    let s = env.stat(&f).await.unwrap();
    assert!(s.is_file && !s.is_dir);
    let s_dir = env.stat(dir.path()).await.unwrap();
    assert!(s_dir.is_dir && !s_dir.is_file);
}

#[tokio::test]
async fn path_exists_returns_false_for_missing() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    assert!(!env.path_exists(&dir.path().join("missing")).await.unwrap());
}

#[tokio::test]
async fn remove_force_skips_missing() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let opts = RemoveOpts {
        recursive: false,
        force: true,
    };
    env.remove(&dir.path().join("nope"), opts).await.unwrap();
}

#[tokio::test]
async fn remove_recursive_clears_dir() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let sub = dir.path().join("sub");
    env.create_dir(&sub, true).await.unwrap();
    env.write_file(&sub.join("f"), b"x").await.unwrap();
    let opts = RemoveOpts {
        recursive: true,
        force: false,
    };
    env.remove(&sub, opts).await.unwrap();
    assert!(!env.path_exists(&sub).await.unwrap());
}

#[tokio::test]
async fn create_temp_dir_creates_unique_paths() {
    let env = LocalEnv::new(std::env::temp_dir());
    let a = env.create_temp_dir(Some("hivecore-test-")).await.unwrap();
    let b = env.create_temp_dir(Some("hivecore-test-")).await.unwrap();
    assert_ne!(a, b);
    let _ = env
        .remove(
            &a,
            RemoveOpts {
                recursive: true,
                force: true,
            },
        )
        .await;
    let _ = env
        .remove(
            &b,
            RemoveOpts {
                recursive: true,
                force: true,
            },
        )
        .await;
}

#[tokio::test]
async fn exec_runs_simple_command() {
    let env = LocalEnv::new(std::env::temp_dir());
    let out = env.exec("echo hello", ExecOpts::default()).await.unwrap();
    assert_eq!(out.exit_code, 0);
    assert!(String::from_utf8_lossy(&out.stdout).trim() == "hello");
}

#[tokio::test]
async fn exec_propagates_exit_code() {
    let env = LocalEnv::new(std::env::temp_dir());
    let out = env.exec("exit 7", ExecOpts::default()).await.unwrap();
    assert_eq!(out.exit_code, 7);
}

#[tokio::test]
async fn exec_honours_timeout() {
    let env = LocalEnv::new(std::env::temp_dir());
    let opts = ExecOpts {
        timeout: Some(std::time::Duration::from_millis(100)),
        ..Default::default()
    };
    let res = env.exec("sleep 5", opts).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn resolve_path_joins_relative_under_cwd() {
    let dir = tempdir().unwrap();
    let env = LocalEnv::new(dir.path().to_path_buf());
    let p = env.resolve_path(std::path::Path::new("foo/bar.txt"));
    assert_eq!(p, dir.path().join("foo/bar.txt"));
}

#[tokio::test]
async fn resolve_path_passes_absolute_through() {
    let env = LocalEnv::new("/tmp");
    let abs = std::path::Path::new("/etc/hostname");
    assert_eq!(
        env.resolve_path(abs),
        std::path::PathBuf::from("/etc/hostname")
    );
}
