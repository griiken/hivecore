//! Default thresholds — jcode constants per ADR-026 §Decisions.
//! All overridable per-agent via TOML; defaults are well-engineered values
//! taken from `1jehuang/jcode` `src/compaction.rs`.

/// Configuration for `SummarizingTransform`. Construct via
/// `CompactionConfig { ..DEFAULT_COMPACTION_CONFIG }`.
#[derive(Debug, Clone, Copy)]
pub struct CompactionConfig {
    /// Total token budget for the model context window. Default: 128k —
    /// matches `gpt-5.4-nano` and similar; override per-agent for larger
    /// or smaller models.
    pub context_window: u32,

    /// Background compaction fires when estimated tokens exceed
    /// `threshold * context_window`. Default 0.80 (jcode).
    pub threshold: f32,

    /// Hard / emergency-drop threshold. Default 0.95 (jcode). At this
    /// level the transform skips summarization and drops the oldest
    /// non-user message synchronously.
    pub critical_threshold: f32,

    /// Reserved tokens for system prompt + tool schemas. Default 16384 (pi
    /// `reserveTokens`). Subtracted from `context_window` for the threshold
    /// calculation.
    pub reserve_tokens: u32,

    /// Number of recent turns to preserve verbatim in the model payload.
    /// Default 10 (jcode `RECENT_TURNS_TO_KEEP`).
    pub recent_turns_to_keep: usize,

    /// Floor — never strip the conversation below this. Default 2 (jcode).
    pub min_turns_to_keep: usize,

    /// Tool result truncation when in emergency mode. Default 4000 chars
    /// (jcode `EMERGENCY_TOOL_RESULT_MAX_CHARS`).
    pub emergency_tool_result_max_chars: usize,
}

pub const DEFAULT_COMPACTION_CONFIG: CompactionConfig = CompactionConfig {
    context_window: 128_000,
    threshold: 0.80,
    critical_threshold: 0.95,
    reserve_tokens: 16_384,
    recent_turns_to_keep: 10,
    min_turns_to_keep: 2,
    emergency_tool_result_max_chars: 4_000,
};

impl Default for CompactionConfig {
    fn default() -> Self {
        DEFAULT_COMPACTION_CONFIG
    }
}

impl CompactionConfig {
    /// Effective budget for conversation tokens (after reserving system
    /// prompt overhead). Used as denominator in the threshold check.
    pub fn effective_window(&self) -> u32 {
        self.context_window.saturating_sub(self.reserve_tokens)
    }

    pub fn threshold_tokens(&self) -> u32 {
        (self.effective_window() as f32 * self.threshold) as u32
    }

    pub fn critical_tokens(&self) -> u32 {
        (self.effective_window() as f32 * self.critical_threshold) as u32
    }
}
