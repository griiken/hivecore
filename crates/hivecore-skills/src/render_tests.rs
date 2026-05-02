use std::path::PathBuf;

use super::*;
use crate::skill::{Skill, SkillFrontmatter};

fn skill(body: &str, args: Vec<&str>) -> Skill {
    Skill {
        name: "demo".into(),
        description: "demo".into(),
        source_path: PathBuf::from("/tmp/demo.md"),
        body_template: body.into(),
        frontmatter: SkillFrontmatter {
            name: "demo".into(),
            description: "demo".into(),
            arguments: args.into_iter().map(String::from).collect(),
            ..Default::default()
        },
    }
}

fn args(positional: &[&str]) -> InvocationArgs {
    InvocationArgs {
        positional: positional.iter().map(|s| s.to_string()).collect(),
        named: Default::default(),
    }
}

#[tokio::test]
async fn expands_positional_shorthand() {
    let s = skill("Hello $0 from $1", vec![]);
    let out = render(&s, &args(&["alice", "bob"]), &RenderCtx::default())
        .await
        .unwrap();
    assert_eq!(out, "Hello alice from bob");
}

#[tokio::test]
async fn expands_named_arguments_via_frontmatter_position() {
    let s = skill("$name was here", vec!["name"]);
    let out = render(&s, &args(&["alice"]), &RenderCtx::default())
        .await
        .unwrap();
    assert_eq!(out, "alice was here");
}

#[tokio::test]
async fn expands_arguments_full_and_indexed() {
    let s = skill("all=[$ARGUMENTS] one=$ARGUMENTS[1]", vec![]);
    let out = render(&s, &args(&["a", "b", "c"]), &RenderCtx::default())
        .await
        .unwrap();
    assert_eq!(out, "all=[a b c] one=b");
}

#[tokio::test]
async fn expands_builtin_session_id() {
    let s = skill("session=${SESSION_ID}", vec![]);
    let ctx = RenderCtx {
        session_id: "sess-123".into(),
        ..RenderCtx::default()
    };
    let out = render(&s, &args(&[]), &ctx).await.unwrap();
    assert_eq!(out, "session=sess-123");
}

#[tokio::test]
async fn errors_on_unknown_builtin() {
    let s = skill("${BADKEY}", vec![]);
    let err = render(&s, &args(&[]), &RenderCtx::default())
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("unknown"));
}

#[tokio::test]
async fn runs_bash_injection() {
    let s = skill("count=!`echo three two one | wc -w`", vec![]);
    let out = render(&s, &args(&[]), &RenderCtx::default()).await.unwrap();
    assert_eq!(out, "count=3");
}

#[tokio::test]
async fn bash_failure_propagates() {
    let s = skill("nope=!`exit 4`", vec![]);
    let err = render(&s, &args(&[]), &RenderCtx::default())
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("exited 4"));
}

#[tokio::test]
async fn variable_then_bash_pipeline() {
    let s = skill("user=$0, host=!`echo localhost`", vec!["user"]);
    let out = render(&s, &args(&["alice"]), &RenderCtx::default())
        .await
        .unwrap();
    assert_eq!(out, "user=alice, host=localhost");
}
