use super::*;
use crate::agent::{Agent, Limits, ModelSelection, ToolMode, ToolPolicy};

fn make(id: &str) -> Agent {
    Agent {
        id: id.into(),
        name: id.into(),
        version: "0.1.0".into(),
        description: String::new(),
        model: ModelSelection {
            provider: "openai".into(),
            id: "gpt-5.4-nano".into(),
        },
        system_prompt: "x".into(),
        tools: ToolPolicy {
            mode: ToolMode::All,
            list: vec![],
        },
        skills: crate::agent::SkillPolicy::All,
        limits: Limits { max_iterations: 8 },
    }
}

#[test]
fn preserves_insertion_order() {
    let r = AgentRegistry::from_agents(vec![make("alpha"), make("bravo")]).unwrap();
    assert_eq!(r.ids(), &["alpha", "bravo"]);
    assert_eq!(r.first().unwrap().id, "alpha");
}

#[test]
fn rejects_duplicate_ids() {
    let err = AgentRegistry::from_agents(vec![make("a"), make("a")]).unwrap_err();
    assert!(format!("{err}").contains("duplicate"));
}

#[test]
fn lookup_by_id() {
    let r = AgentRegistry::from_agents(vec![make("x")]).unwrap();
    assert!(r.get("x").is_some());
    assert!(r.get("y").is_none());
}
