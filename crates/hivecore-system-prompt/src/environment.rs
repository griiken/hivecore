//! `EnvironmentContextSource` — Codex's `<environment_context>` section.
//!
//! Format mirrors `gpt_5_2_prompt.md`'s expected env block:
//!
//! ```text
//! <environment_context>
//!   <cwd>/home/user/code</cwd>
//!   <shell>zsh</shell>
//!   <date>2026-04-30</date>
//! </environment_context>
//! ```

use async_trait::async_trait;
use chrono::Utc;

use crate::error::PromptError;
use crate::source::{ContextSource, RoleHint, SourceFragment};

#[derive(Debug, Clone)]
pub struct EnvironmentContextSource {
    pub cwd: std::path::PathBuf,
    pub shell: Option<String>,
    pub include_date: bool,
}

impl EnvironmentContextSource {
    pub fn new(cwd: impl Into<std::path::PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            shell: detect_shell(),
            include_date: true,
        }
    }
}

fn detect_shell() -> Option<String> {
    std::env::var("SHELL").ok().and_then(|s| {
        std::path::Path::new(&s)
            .file_name()
            .and_then(|n| n.to_str().map(String::from))
    })
}

#[async_trait]
impl ContextSource for EnvironmentContextSource {
    fn name(&self) -> &str {
        "environment_context"
    }
    async fn fragment(&self) -> Result<Option<SourceFragment>, PromptError> {
        let mut body = String::from("<environment_context>\n");
        body.push_str(&format!("  <cwd>{}</cwd>\n", self.cwd.display()));
        if let Some(s) = &self.shell {
            body.push_str(&format!("  <shell>{s}</shell>\n"));
        }
        if self.include_date {
            body.push_str(&format!(
                "  <date>{}</date>\n",
                Utc::now().format("%Y-%m-%d")
            ));
        }
        body.push_str("</environment_context>");

        Ok(Some(SourceFragment {
            source: "environment_context".into(),
            role_hint: RoleHint::User,
            body,
        }))
    }
}
