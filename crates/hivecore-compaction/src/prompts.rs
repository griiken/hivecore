//! Vendored prompts. See `prompts/README.md` for upstream attribution.

/// Pi-mono `SUMMARIZATION_PROMPT` — structured-Markdown form. MIT licence.
pub const SUMMARY_PROMPT: &str = include_str!("../prompts/summary.md");

/// Codex `summary_prefix.md` — preamble injected ahead of the summary when
/// the next-turn LLM reads it. Apache-2.0 licence.
pub const SUMMARY_PREFIX: &str = include_str!("../prompts/summary_prefix.md");
