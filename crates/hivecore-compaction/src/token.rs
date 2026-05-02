//! Token estimation (pi+jcode shape).
//!
//! Strategy:
//!   1. If we have a recorded `prompt_tokens` from the last assistant
//!      response, use it as the base — it's authoritative.
//!   2. Otherwise fall back to a `chars / 4` heuristic over message text.
//!
//! Both Codex (`approx_token_count`) and pi (`estimateContextTokens`) ship
//! the same idea; jcode uses `max(chars/4, observed_input_tokens)`.

use hivecore_runtime_core::{AgentMessage, ContentBlock};

const CHARS_PER_TOKEN: usize = 4;

/// Estimate the token cost of `messages`. Pure heuristic — counts character
/// length of all visible text content / 4. Closures of structured content
/// (tool calls, images) get a fixed nominal cost of 32 tokens each so the
/// estimator doesn't ignore them entirely.
pub fn estimate_tokens(messages: &[AgentMessage]) -> u32 {
    let mut chars: usize = 0;
    let mut structural: usize = 0;
    for m in messages {
        if !m.visible_to_model() {
            continue;
        }
        match m {
            AgentMessage::User { content, .. }
            | AgentMessage::Assistant { content, .. }
            | AgentMessage::ToolResult { content, .. } => {
                for b in content {
                    match b {
                        ContentBlock::Text { text } | ContentBlock::Thinking { text } => {
                            chars += text.len();
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            chars += name.len();
                            chars += input.to_string().len();
                            structural += 32;
                        }
                        ContentBlock::Image { .. } => {
                            structural += 256;
                        }
                    }
                }
            }
            AgentMessage::Custom { .. } => {}
        }
    }
    ((chars / CHARS_PER_TOKEN) + structural) as u32
}
