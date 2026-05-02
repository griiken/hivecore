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
