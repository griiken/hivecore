use std::fs;

use tempfile::TempDir;

use super::*;

const VALID: &str = "---
name: greet
description: Say hello to a name
arguments: [name]
---
Hello, $name! From skill $0.
";

fn write(d: &std::path::Path, name: &str, body: &str) {
    fs::create_dir_all(d.parent().unwrap_or(d)).unwrap();
    fs::write(d.join(name), body).unwrap();
}

#[test]
fn loads_one_skill() {
    let d = TempDir::new().unwrap();
    write(d.path(), "greet.md", VALID);
    let skills = SkillLoader::new().add_dir(d.path()).load().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "greet");
    assert_eq!(skills[0].frontmatter.arguments, vec!["name"]);
}

#[test]
fn loads_nested_dir_layout() {
    let d = TempDir::new().unwrap();
    let nested = d.path().join("greet");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("SKILL.md"), VALID).unwrap();
    let skills = SkillLoader::new().add_dir(d.path()).load().unwrap();
    assert_eq!(skills.len(), 1);
}

#[test]
fn skips_md_files_without_frontmatter() {
    let d = TempDir::new().unwrap();
    write(d.path(), "readme.md", "# just a readme\n");
    let skills = SkillLoader::new().add_dir(d.path()).load().unwrap();
    assert!(skills.is_empty());
}

#[test]
fn rejects_invalid_name() {
    let d = TempDir::new().unwrap();
    let bad = VALID.replace("name: greet", "name: Greeter!");
    write(d.path(), "bad.md", &bad);
    let err = SkillLoader::new().add_dir(d.path()).load().unwrap_err();
    assert!(matches!(err, SkillError::InvalidName(_)));
}

#[test]
fn skips_hidden_dirs() {
    let d = TempDir::new().unwrap();
    let hidden = d.path().join(".cache");
    fs::create_dir_all(&hidden).unwrap();
    fs::write(hidden.join("x.md"), VALID).unwrap();
    let skills = SkillLoader::new().add_dir(d.path()).load().unwrap();
    assert!(skills.is_empty());
}
