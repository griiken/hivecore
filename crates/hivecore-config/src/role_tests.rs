use super::*;

fn role(name: &str) -> Role {
    Role {
        name: name.into(),
        description: format!("{name} role"),
        prompt: format!("you are {name}"),
        tools: None,
        model: None,
    }
}

#[test]
fn resolve_call_wins_over_session() {
    let c = role("call");
    let s = role("session");
    let a = role("agent");
    let r = Role::resolve(Some(&c), Some(&s), Some(&a)).unwrap();
    assert_eq!(r.name, "call");
}

#[test]
fn resolve_session_wins_over_agent() {
    let s = role("session");
    let a = role("agent");
    let r = Role::resolve(None, Some(&s), Some(&a)).unwrap();
    assert_eq!(r.name, "session");
}

#[test]
fn resolve_agent_default_used_when_others_absent() {
    let a = role("agent");
    let r = Role::resolve(None, None, Some(&a)).unwrap();
    assert_eq!(r.name, "agent");
}

#[test]
fn resolve_returns_none_when_no_overlay() {
    assert!(Role::resolve(None, None, None).is_none());
}

#[test]
fn resolve_skips_none_in_precedence_order() {
    let s = role("session");
    let a = role("agent");
    // call=None, session=Some, agent=Some ⇒ session wins.
    let r = Role::resolve(None, Some(&s), Some(&a)).unwrap();
    assert_eq!(r.name, "session");
}
