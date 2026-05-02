use std::path::PathBuf;

use super::*;
use crate::skill::{Skill, SkillFrontmatter};

fn make(name: &str, disable: bool) -> Skill {
    Skill {
        name: name.into(),
        description: "x".into(),
        source_path: PathBuf::from(format!("/tmp/{name}.md")),
        body_template: String::new(),
        frontmatter: SkillFrontmatter {
            name: name.into(),
            description: "x".into(),
            disable_model_invocation: disable,
            ..Default::default()
        },
    }
}

#[test]
fn rejects_duplicate_names() {
    let err = SkillRegistry::from_skills(vec![make("a", false), make("a", false)]).unwrap_err();
    assert!(matches!(err, SkillError::Duplicate { .. }));
}

#[test]
fn preserves_insertion_order() {
    let r = SkillRegistry::from_skills(vec![make("alpha", false), make("bravo", false)]).unwrap();
    assert_eq!(r.names(), &["alpha", "bravo"]);
}

#[test]
fn model_invokable_filters_disabled() {
    let r =
        SkillRegistry::from_skills(vec![make("public", false), make("manual_only", true)]).unwrap();
    let tools = r.model_invokable_tools(RenderCtx::default);
    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert_eq!(names, vec!["public"]);
}
