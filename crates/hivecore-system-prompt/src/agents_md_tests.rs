use std::fs;

use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn picks_up_root_agents_md() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("AGENTS.md"), "root rules").unwrap();
    let src = AgentsMdSource::new(d.path(), d.path());
    let f = src.fragment().await.unwrap().unwrap();
    assert!(f.body.contains("root rules"));
    assert!(f.body.contains("<agents_md path="));
}

#[tokio::test]
async fn walks_root_to_cwd_in_order() {
    let d = TempDir::new().unwrap();
    let sub = d.path().join("crates/foo");
    fs::create_dir_all(&sub).unwrap();
    fs::write(d.path().join("AGENTS.md"), "ROOT").unwrap();
    fs::write(d.path().join("crates/AGENTS.md"), "CRATES").unwrap();
    fs::write(sub.join("AGENTS.md"), "FOO").unwrap();

    let src = AgentsMdSource::new(d.path(), &sub);
    let body = src.fragment().await.unwrap().unwrap().body;
    let root_pos = body.find("ROOT").unwrap();
    let crates_pos = body.find("CRATES").unwrap();
    let foo_pos = body.find("FOO").unwrap();
    assert!(root_pos < crates_pos);
    assert!(crates_pos < foo_pos);
}

#[tokio::test]
async fn override_takes_precedence() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("AGENTS.md"), "BASE").unwrap();
    fs::write(d.path().join("AGENTS.override.md"), "OVERRIDE").unwrap();
    let src = AgentsMdSource::new(d.path(), d.path());
    let body = src.fragment().await.unwrap().unwrap().body;
    let ovr = body.find("OVERRIDE").unwrap();
    let base = body.find("BASE").unwrap();
    assert!(ovr < base);
}

#[tokio::test]
async fn truncates_at_budget() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("AGENTS.md"), "x".repeat(2000)).unwrap();
    let src = AgentsMdSource::new(d.path(), d.path()).with_budget(100);
    let body = src.fragment().await.unwrap().unwrap().body;
    assert!(body.contains("truncated to budget"));
}

#[tokio::test]
async fn returns_none_when_no_files() {
    let d = TempDir::new().unwrap();
    let src = AgentsMdSource::new(d.path(), d.path());
    assert!(src.fragment().await.unwrap().is_none());
}
