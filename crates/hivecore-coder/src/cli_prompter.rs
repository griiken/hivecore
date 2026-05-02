//! CLI `Approval` sink — reads the user's choice from stdin.
//!
//! Single-line prompt; non-blocking on the async runtime via
//! `tokio::task::spawn_blocking` so the event loop keeps draining.

use std::io::{self, BufRead, Write};

use async_trait::async_trait;
use hivecore_runtime_core::{Approval, ApprovalAction, ApprovalDecision, ApprovalRequest};

#[derive(Debug, Default)]
pub struct CliPrompter;

#[async_trait]
impl Approval for CliPrompter {
    async fn request(&self, req: ApprovalRequest<'_>) -> ApprovalDecision {
        let header = match req.action {
            ApprovalAction::Tool {
                tool_name,
                input_preview,
                ..
            } => format!(
                "tool: {tool_name}\ninput: {}",
                serde_json::to_string(input_preview).unwrap_or_default()
            ),
            ApprovalAction::ApplyPatch { root, files, .. } => format!(
                "apply_patch under {} ({} file{})",
                root.display(),
                files.len(),
                if files.len() == 1 { "" } else { "s" }
            ),
            ApprovalAction::Mcp {
                server,
                tool_name,
                input_preview,
            } => format!(
                "mcp: {server}/{tool_name}\ninput: {}",
                serde_json::to_string(input_preview).unwrap_or_default()
            ),
        };
        let reason = req
            .reason
            .clone()
            .unwrap_or_else(|| "(no reason given)".to_string());
        let explicit_yes = req.explicit_yes_required;

        tokio::task::spawn_blocking(move || {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let _ = writeln!(out, "\n── approval required ──");
            let _ = writeln!(out, "reason: {reason}");
            let _ = writeln!(out, "{header}");
            if explicit_yes {
                let _ = writeln!(
                    out,
                    "DESTRUCTIVE: type literal 'y' to allow once  [s]ession  [q]uit  (anything else denies)"
                );
            } else {
                let _ = writeln!(out, "[a]llow once  [s]ession allow  [d]eny  [q]uit");
            }
            let _ = write!(out, "> ");
            let _ = out.flush();
            drop(out);

            let stdin = io::stdin();
            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() {
                return ApprovalDecision::Abort;
            }
            let trimmed = line.trim();
            let first = trimmed.chars().next();
            // Aider `explicit_yes_required`: literal 'y' only — Enter / 'a' / etc. all deny.
            if explicit_yes {
                return match first {
                    Some('y') => ApprovalDecision::Approved,
                    Some('s') | Some('S') => ApprovalDecision::ApprovedForSession,
                    Some('q') | Some('Q') => ApprovalDecision::Abort,
                    _ => ApprovalDecision::Denied,
                };
            }
            match first.unwrap_or('d') {
                'a' | 'A' | 'y' | 'Y' => ApprovalDecision::Approved,
                's' | 'S' => ApprovalDecision::ApprovedForSession,
                'q' | 'Q' => ApprovalDecision::Abort,
                _ => ApprovalDecision::Denied,
            }
        })
        .await
        .unwrap_or(ApprovalDecision::Abort)
    }
}
