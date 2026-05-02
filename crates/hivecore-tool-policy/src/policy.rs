//! Top-level dial. Mirrors Codex `AskForApproval`
//! (`codex-rs/protocol/src/protocol.rs:936-1006`) minus the deprecated
//! `OnFailure` variant and the `Granular` per-category mode (v0.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Ask only for actions the matcher classifies as risky. Default.
    #[default]
    OnRequest,
    /// Auto-allow only `RiskHint::read_only=true` actions; ask on everything else.
    UnlessTrusted,
    /// Never ask. The matcher still runs (useful for typed deny-list);
    /// matched calls become an error returned to the model.
    Never,
    /// Headless / non-interactive mode (CI). Matched-but-needs-Ask calls
    /// fail with an error rather than invoking the sink — matches Continue.dev
    /// `--print` mode behaviour where `ask` becomes process exit. Without
    /// this, an `Approval` sink in CI hangs forever waiting on stdin.
    FailOnAsk,
}
