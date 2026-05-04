use std::fs;

use tempfile::TempDir;

use super::*;

const VALID_TOML: &str = r#"
[agent]
id = "default"
name = "Default"
version = "0.1.0"

[agent.model]
provider = "openai"
id = "gpt-5.4-nano"

[agent.system_prompt]
inline = "you are concise"

[agent.tools]
mode = "allowlist"
list = ["read_file", "bash"]

[agent.limits]
max_iterations = 16
"#;

fn write(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn loads_a_valid_agent() {
    let d = TempDir::new().unwrap();
    let p = write(d.path(), "default.toml", VALID_TOML);
    let agent = AgentLoader::new().load_file(&p).unwrap();
    assert_eq!(agent.id, "default");
    assert_eq!(agent.model.id, "gpt-5.4-nano");
    assert_eq!(agent.tools.mode, ToolMode::Allowlist);
    assert!(agent.tools.allows("read_file"));
    assert!(!agent.tools.allows("write_file"));
}

#[test]
fn rejects_invalid_id() {
    let d = TempDir::new().unwrap();
    let bad = VALID_TOML.replace(r#"id = "default""#, r#"id = "Default!""#);
    let p = write(d.path(), "x.toml", &bad);
    let err = AgentLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Validation(ValidationError::InvalidId(_))
    ));
}

#[test]
fn rejects_both_inline_and_path() {
    let d = TempDir::new().unwrap();
    let bad = VALID_TOML.replace(
        r#"inline = "you are concise""#,
        "inline = \"x\"\npath = \"y.md\"",
    );
    let p = write(d.path(), "x.toml", &bad);
    let err = AgentLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Validation(ValidationError::PromptSourceAmbiguous)
    ));
}

#[test]
fn resolves_prompt_path_relative_to_agent_file() {
    let d = TempDir::new().unwrap();
    fs::write(d.path().join("body.md"), "you are quiet").unwrap();
    let body = VALID_TOML.replace(r#"inline = "you are concise""#, r#"path = "body.md""#);
    let p = write(d.path(), "x.toml", &body);
    let agent = AgentLoader::new().load_file(&p).unwrap();
    assert_eq!(agent.system_prompt, "you are quiet");
}

#[test]
fn rejects_all_mode_with_nonempty_list() {
    let d = TempDir::new().unwrap();
    let bad = VALID_TOML.replace(r#"mode = "allowlist""#, r#"mode = "all""#);
    let p = write(d.path(), "x.toml", &bad);
    let err = AgentLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Validation(ValidationError::ToolListWithAllMode)
    ));
}

#[test]
fn rejects_unsupported_provider() {
    let d = TempDir::new().unwrap();
    let bad = VALID_TOML.replace(r#"provider = "openai""#, r#"provider = "ollama""#);
    let p = write(d.path(), "x.toml", &bad);
    let err = AgentLoader::new().load_file(&p).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Validation(ValidationError::UnsupportedProvider(_))
    ));
}

#[test]
fn load_dir_picks_up_only_toml_files() {
    let d = TempDir::new().unwrap();
    write(d.path(), "default.toml", VALID_TOML);
    write(d.path(), "ignored.md", "# not an agent");
    let agents = AgentLoader::new().load_dir(d.path()).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].id, "default");
}

#[test]
fn defaults_skills_to_all_when_field_omitted() {
    let d = TempDir::new().unwrap();
    let p = write(d.path(), "default.toml", VALID_TOML);
    let agent = AgentLoader::new().load_file(&p).unwrap();
    assert!(matches!(agent.skills, SkillPolicy::All));
    assert!(agent.skills.allows("any-skill"));
}

#[test]
fn parses_star_skills_as_all() {
    let d = TempDir::new().unwrap();
    let body = VALID_TOML.replace("version = \"0.1.0\"", "version = \"0.1.0\"\nskills = \"*\"");
    let p = write(d.path(), "x.toml", &body);
    let agent = AgentLoader::new().load_file(&p).unwrap();
    assert!(matches!(agent.skills, SkillPolicy::All));
}

#[test]
fn parses_list_skills_as_allowlist() {
    let d = TempDir::new().unwrap();
    let body = VALID_TOML.replace(
        "version = \"0.1.0\"",
        "version = \"0.1.0\"\nskills = [\"pdf\", \"review\"]",
    );
    let p = write(d.path(), "x.toml", &body);
    let agent = AgentLoader::new().load_file(&p).unwrap();
    assert!(agent.skills.allows("pdf"));
    assert!(agent.skills.allows("review"));
    assert!(!agent.skills.allows("dangerous"));
}

#[test]
fn load_dir_handles_empty_or_missing_dir() {
    let d = TempDir::new().unwrap();
    assert!(AgentLoader::new().load_dir(d.path()).unwrap().is_empty());
    let nope = d.path().join("nonexistent");
    assert!(AgentLoader::new().load_dir(&nope).unwrap().is_empty());
}
