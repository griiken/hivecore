//! Tool-name qualification per ADR-027 §6.
//!
//! Algorithm adopted verbatim from openai/codex `qualify_tools`:
//!
//! - Final qualified name: `mcp__<sanitized_server>__<sanitized_tool>`.
//! - `MAX_TOOL_NAME_LENGTH = 64` bytes.
//! - Sanitize: lowercase ASCII alphanumeric kept; everything else → `-`; trim
//!   leading/trailing `-`; empty → `"app"`; finally replace `-` with `_`. Final
//!   alphabet: `[a-z0-9_]`.
//! - Length overflow OR cross-server collision: append the first 12 hex chars
//!   of `SHA-1(raw_identity)`. Truncate `tool_name` first; only suffix the
//!   namespace when the tool already maxed out.
//!
//! `ToolFilter { enabled, disabled }` (defined in `config`) operates on the
//! *raw* server-native tool names — applied **before** qualification.

use sha1::{Digest, Sha1};

pub const NAMESPACE_PREFIX: &str = "mcp__";
pub const MAX_TOOL_NAME_LENGTH: usize = 64;
const SUFFIX_LEN: usize = 12;

/// Sanitize an arbitrary string into the Codex tool-name alphabet
/// (`[a-z0-9_]`). Empty input becomes `"app"`.
pub fn sanitize(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out: String = lower
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = out.trim_matches('-').to_string();
    out = if trimmed.is_empty() {
        "app".to_string()
    } else {
        trimmed
    };
    out.replace('-', "_")
}

/// First 12 hex chars of SHA-1 of the raw identity. Used as a stable suffix
/// for length-overflow / collision disambiguation.
fn sha1_suffix(raw_identity: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(raw_identity.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)[..SUFFIX_LEN].to_string()
}

/// Qualify a `(server, tool)` pair into the Codex namespaced form. If the
/// total length would exceed `MAX_TOOL_NAME_LENGTH`, the tool component is
/// truncated and a SHA-1 suffix appended (raw identity = `server::tool`).
/// If even with a fully-truncated tool name the result is still too long,
/// the *server* component receives the suffix instead.
pub fn qualify_tool_name(server: &str, tool: &str) -> String {
    let s_server = sanitize(server);
    let s_tool = sanitize(tool);
    let raw = format!("{server}::{tool}");

    let prefix_len = NAMESPACE_PREFIX.len() + s_server.len() + 2; // "mcp__<server>__"
    let budget_for_tool = MAX_TOOL_NAME_LENGTH.saturating_sub(prefix_len);

    if s_tool.len() <= budget_for_tool {
        return format!("{NAMESPACE_PREFIX}{s_server}__{s_tool}");
    }

    // Need disambiguation suffix. Try truncating tool first.
    let suffix = sha1_suffix(&raw);
    let suffix_with_sep = format!("_{suffix}");
    if budget_for_tool > suffix_with_sep.len() {
        let keep = budget_for_tool - suffix_with_sep.len();
        let truncated = &s_tool[..keep.min(s_tool.len())];
        return format!("{NAMESPACE_PREFIX}{s_server}__{truncated}{suffix_with_sep}");
    }

    // Tool budget too small — truncate server instead.
    let server_budget = MAX_TOOL_NAME_LENGTH
        .saturating_sub(NAMESPACE_PREFIX.len() + 2 + s_tool.len() + suffix_with_sep.len());
    let truncated_server = &s_server[..server_budget.min(s_server.len())];
    format!("{NAMESPACE_PREFIX}{truncated_server}{suffix_with_sep}__{s_tool}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_alnum_and_underscores() {
        assert_eq!(sanitize("Hello-World"), "hello_world");
        assert_eq!(sanitize("foo.bar/baz"), "foo_bar_baz");
        assert_eq!(sanitize("--leading"), "leading");
        assert_eq!(sanitize("trailing--"), "trailing");
        assert_eq!(sanitize(""), "app");
        assert_eq!(sanitize("###"), "app");
        assert_eq!(sanitize("CamelCase"), "camelcase");
    }

    #[test]
    fn qualify_short_names_unchanged() {
        let q = qualify_tool_name("github", "create_issue");
        assert_eq!(q, "mcp__github__create_issue");
        assert!(q.len() <= MAX_TOOL_NAME_LENGTH);
    }

    #[test]
    fn qualify_sanitizes_components() {
        let q = qualify_tool_name("My-Server", "Foo.Bar");
        assert_eq!(q, "mcp__my_server__foo_bar");
    }

    #[test]
    fn qualify_truncates_long_tool_with_sha_suffix() {
        let server = "srv";
        let tool = "a".repeat(80);
        let q = qualify_tool_name(server, &tool);
        assert!(q.len() <= MAX_TOOL_NAME_LENGTH);
        assert!(q.starts_with("mcp__srv__"));
        // Suffix must be 12 hex chars, prefixed by `_`.
        let tail: &str = &q[q.len() - SUFFIX_LEN..];
        assert!(tail.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn qualify_stable_suffix_for_same_inputs() {
        let tool = "z".repeat(80);
        let a = qualify_tool_name("srv", &tool);
        let b = qualify_tool_name("srv", &tool);
        assert_eq!(a, b);
    }

    #[test]
    fn qualify_different_raw_inputs_diverge_in_suffix() {
        let a = qualify_tool_name("srv", &"a".repeat(80));
        let b = qualify_tool_name("srv", &"b".repeat(80));
        assert_ne!(a, b);
    }
}
