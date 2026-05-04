use super::*;

fn policy(mode: ToolMode, list: &[&str]) -> ToolPolicy {
    ToolPolicy {
        mode,
        list: list.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn allowlist_admits_only_listed_tools() {
    let p = policy(ToolMode::Allowlist, &["read_file", "grep"]);
    assert!(p.allows("read_file"));
    assert!(p.allows("grep"));
    assert!(!p.allows("bash"));
}

#[test]
fn all_mode_admits_anything() {
    let p = policy(ToolMode::All, &[]);
    assert!(p.allows("anything"));
    assert!(p.allows("at_all"));
}

#[test]
fn denylist_blocks_only_listed() {
    let p = policy(ToolMode::Denylist, &["bash"]);
    assert!(p.allows("read_file"));
    assert!(!p.allows("bash"));
}

#[test]
fn filter_preserves_input_order() {
    let p = policy(ToolMode::Allowlist, &["b", "d"]);
    let out = p.filter(["a", "b", "c", "d"]);
    assert_eq!(out, vec!["b", "d"]);
}

#[test]
fn skill_policy_default_is_all() {
    let p = SkillPolicy::default();
    assert!(p.allows("anything"));
    assert!(p.allows("at-all"));
}

#[test]
fn skill_policy_allowlist_admits_only_listed() {
    let p = SkillPolicy::Allow(vec!["pdf".into(), "review".into()]);
    assert!(p.allows("pdf"));
    assert!(p.allows("review"));
    assert!(!p.allows("dangerous"));
}

#[test]
fn skill_policy_filter_preserves_input_order() {
    let p = SkillPolicy::Allow(vec!["b".into(), "d".into()]);
    let out = p.filter(["a", "b", "c", "d"]);
    assert_eq!(out, vec!["b", "d"]);
}

#[test]
fn skill_policy_deserialises_star_as_all() {
    let p: SkillPolicy = toml::from_str("skills = \"*\"\n")
        .map(|v: toml::Value| {
            v.get("skills")
                .cloned()
                .unwrap()
                .try_into::<SkillPolicy>()
                .unwrap()
        })
        .unwrap();
    assert!(matches!(p, SkillPolicy::All));
}

#[test]
fn skill_policy_deserialises_list_as_allowlist() {
    let v: toml::Value = toml::from_str("skills = [\"a\", \"b\"]\n").unwrap();
    let p: SkillPolicy = v.get("skills").cloned().unwrap().try_into().unwrap();
    assert!(matches!(p, SkillPolicy::Allow(ref l) if l == &["a", "b"]));
}

#[test]
fn skill_policy_rejects_garbage_string() {
    let v: toml::Value = toml::from_str("skills = \"all\"\n").unwrap();
    let err = v
        .get("skills")
        .cloned()
        .unwrap()
        .try_into::<SkillPolicy>()
        .unwrap_err();
    assert!(err.to_string().contains("\"*\""));
}
