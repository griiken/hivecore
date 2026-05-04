use std::fs;

use tempfile::TempDir;

use super::*;

fn write_role(dir: &TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.path().join(format!("{name}.toml"));
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn loads_minimal_role() {
    let dir = TempDir::new().unwrap();
    let p = write_role(
        &dir,
        "researcher",
        r#"name = "researcher"
description = "Read-only investigation."
prompt = "You are operating in researcher mode."
"#,
    );
    let r = RoleLoader::new().load_file(&p).unwrap();
    assert_eq!(r.name, "researcher");
    assert_eq!(r.description, "Read-only investigation.");
    assert_eq!(r.prompt, "You are operating in researcher mode.");
    assert!(r.tools.is_none());
    assert!(r.model.is_none());
}

#[test]
fn loads_role_with_tool_override() {
    let dir = TempDir::new().unwrap();
    let p = write_role(
        &dir,
        "researcher",
        r#"name = "researcher"
description = ""
prompt = "research only"
tools = ["read_file", "grep"]
"#,
    );
    let r = RoleLoader::new().load_file(&p).unwrap();
    assert_eq!(
        r.tools.as_deref(),
        Some(&["read_file".into(), "grep".into()][..])
    );
}

#[test]
fn loads_role_with_model_override() {
    let dir = TempDir::new().unwrap();
    let p = write_role(
        &dir,
        "lite",
        r#"name = "lite"
description = ""
prompt = "use cheap model"
model = "openai/gpt-5.4-nano"
"#,
    );
    let r = RoleLoader::new().load_file(&p).unwrap();
    assert_eq!(r.model.as_deref(), Some("openai/gpt-5.4-nano"));
}

#[test]
fn rejects_empty_prompt() {
    let dir = TempDir::new().unwrap();
    let p = write_role(
        &dir,
        "bad",
        r#"name = "bad"
description = ""
prompt = "   "
"#,
    );
    let err = RoleLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        crate::error::ConfigError::Validation(crate::error::ValidationError::EmptyRolePrompt)
    ));
}

#[test]
fn rejects_invalid_name() {
    let dir = TempDir::new().unwrap();
    let p = write_role(
        &dir,
        "bad",
        r#"name = "Bad-Caps"
description = ""
prompt = "x"
"#,
    );
    let err = RoleLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        crate::error::ConfigError::Validation(crate::error::ValidationError::InvalidId(_))
    ));
}

#[test]
fn load_dir_picks_up_all_toml_files() {
    let dir = TempDir::new().unwrap();
    write_role(
        &dir,
        "a",
        r#"name = "a"
description = ""
prompt = "alpha"
"#,
    );
    write_role(
        &dir,
        "b",
        r#"name = "b"
description = ""
prompt = "beta"
"#,
    );
    fs::write(dir.path().join("notes.md"), "ignored").unwrap();
    let roles = RoleLoader::new().load_dir(dir.path()).unwrap();
    assert_eq!(roles.len(), 2);
}

#[test]
fn load_dir_returns_empty_for_missing_dir() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("does-not-exist");
    let roles = RoleLoader::new().load_dir(&missing).unwrap();
    assert!(roles.is_empty());
}
