//! Render a skill body: variable substitution + `!`<cmd>`` bash injection.
//!
//! Variable forms:
//!   - `$ARGUMENTS`            — full argument string (joined with spaces)
//!   - `$ARGUMENTS[N]`         — Nth positional argument (0-based)
//!   - `$N`                    — shorthand for `$ARGUMENTS[N]`
//!   - `$<named>`              — named argument from frontmatter `arguments:`
//!   - `${SKILL_DIR}`          — directory containing the skill's `.md` file
//!   - `${SESSION_ID}`         — passed in via `RenderCtx`
//!
//! Bash injection: `` !`<cmd>` `` runs the command before the body is
//! returned to the model. Output replaces the placeholder. Errors fail the
//! render — the model never sees the broken placeholder.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use regex::Regex;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::SkillError;
use crate::skill::Skill;

#[derive(Debug, Clone)]
pub struct RenderCtx {
    pub session_id: String,
    pub effort: Option<String>,
    pub workdir: PathBuf,
    pub bash_timeout_ms: u64,
}

impl Default for RenderCtx {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            effort: None,
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bash_timeout_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InvocationArgs {
    pub positional: Vec<String>,
    pub named: HashMap<String, String>,
}

impl InvocationArgs {
    pub fn from_json(value: &serde_json::Value) -> Result<Self, SkillError> {
        let map = value
            .as_object()
            .ok_or_else(|| SkillError::InvalidArgs("expected object".into()))?;
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        // `positional` key is honoured if present, otherwise treat all
        // string-valued keys as named args.
        if let Some(pos) = map.get("positional").and_then(|v| v.as_array()) {
            for v in pos {
                positional.push(value_to_string(v));
            }
        }
        for (k, v) in map {
            if k == "positional" {
                continue;
            }
            named.insert(k.clone(), value_to_string(v));
        }
        Ok(Self { positional, named })
    }
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub async fn render(
    skill: &Skill,
    args: &InvocationArgs,
    ctx: &RenderCtx,
) -> Result<String, SkillError> {
    let with_vars = expand_variables(skill, args, ctx)?;
    let with_bash = expand_bash(&with_vars, ctx).await?;
    Ok(with_bash)
}

fn expand_variables(
    skill: &Skill,
    args: &InvocationArgs,
    ctx: &RenderCtx,
) -> Result<String, SkillError> {
    let body = &skill.body_template;

    // Pull frontmatter argument-name → position mapping.
    let positional_lookup = |idx: usize| -> Option<&String> { args.positional.get(idx) };
    let named_lookup = |name: &str| -> Option<String> {
        // First explicit named arg…
        if let Some(v) = args.named.get(name) {
            return Some(v.clone());
        }
        // …otherwise check frontmatter `arguments:` index mapping.
        if let Some(pos) = skill.frontmatter.arguments.iter().position(|n| n == name) {
            return positional_lookup(pos).cloned();
        }
        None
    };

    // 1. ${BUILTIN}
    let builtin_re = Regex::new(r"\$\{([A-Z_]+)\}").unwrap();
    let mut out = String::with_capacity(body.len());
    let mut last_end = 0;
    for cap in builtin_re.captures_iter(body) {
        let m = cap.get(0).unwrap();
        out.push_str(&body[last_end..m.start()]);
        let key = cap.get(1).unwrap().as_str();
        let resolved = match key {
            "SKILL_DIR" => skill
                .source_path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            "SESSION_ID" => ctx.session_id.clone(),
            "EFFORT" => ctx.effort.clone().unwrap_or_default(),
            other => return Err(SkillError::BadVariable(format!("unknown ${{{other}}}"))),
        };
        out.push_str(&resolved);
        last_end = m.end();
    }
    out.push_str(&body[last_end..]);

    // 2. $ARGUMENTS[N]
    let bracket_re = Regex::new(r"\$ARGUMENTS\[(\d+)\]").unwrap();
    let out = bracket_re
        .replace_all(&out, |c: &regex::Captures<'_>| {
            let n: usize = c[1].parse().unwrap_or(usize::MAX);
            positional_lookup(n).cloned().unwrap_or_default()
        })
        .into_owned();

    // 3. $ARGUMENTS  (full joined string)
    let full_re = Regex::new(r"\$ARGUMENTS\b").unwrap();
    let out = full_re
        .replace_all(&out, args.positional.join(" "))
        .into_owned();

    // 4. $N — pure-numeric positional shorthand. Out-of-range references
    //    are left *literal* so bash injections like `awk '{print $1}'`
    //    survive when no positional args were supplied.
    let num_re = Regex::new(r"\$(\d+)").unwrap();
    let out = num_re
        .replace_all(&out, |c: &regex::Captures<'_>| {
            let n: usize = c[1].parse().unwrap_or(usize::MAX);
            match positional_lookup(n) {
                Some(v) => v.clone(),
                None => format!("${n}"),
            }
        })
        .into_owned();

    // 5. $<name> — named from frontmatter or args. Unknown names are left
    //    literal too so `$HOME`, `$PATH`, etc. survive into bash injections.
    let named_re = Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    let out = named_re
        .replace_all(&out, |c: &regex::Captures<'_>| {
            let key = &c[1];
            match named_lookup(key) {
                Some(v) => v,
                None => format!("${key}"),
            }
        })
        .into_owned();

    Ok(out)
}

/// Match `` !`<cmd>` `` (a backticked shell command preceded by `!`).
async fn expand_bash(text: &str, ctx: &RenderCtx) -> Result<String, SkillError> {
    let re = Regex::new(r"!`([^`]+)`").unwrap();
    let mut out = String::with_capacity(text.len());
    let mut last_end = 0;
    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        out.push_str(&text[last_end..m.start()]);
        let cmd = cap.get(1).unwrap().as_str();
        let result = run_bash(cmd, ctx).await?;
        out.push_str(&result);
        last_end = m.end();
    }
    out.push_str(&text[last_end..]);
    Ok(out)
}

async fn run_bash(cmd: &str, ctx: &RenderCtx) -> Result<String, SkillError> {
    let mut child = Command::new("bash")
        .arg("-lc")
        .arg(cmd)
        .current_dir(&ctx.workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| SkillError::BashFailed(e.to_string()))?;

    let mut stdout = child.stdout.take().expect("piped");
    let dur = Duration::from_millis(ctx.bash_timeout_ms);
    let waited: Result<(Vec<u8>, std::process::ExitStatus), SkillError> = timeout(dur, async {
        let mut buf = Vec::new();
        stdout
            .read_to_end(&mut buf)
            .await
            .map_err(|e| SkillError::BashFailed(e.to_string()))?;
        let status = child
            .wait()
            .await
            .map_err(|e| SkillError::BashFailed(e.to_string()))?;
        Ok::<_, SkillError>((buf, status))
    })
    .await
    .map_err(|_| SkillError::BashFailed(format!("timeout after {}ms", dur.as_millis())))?;

    let (buf, status) = waited?;
    if !status.success() {
        return Err(SkillError::BashFailed(format!(
            "command `{}` exited {}",
            cmd,
            status.code().unwrap_or(-1)
        )));
    }
    Ok(String::from_utf8_lossy(&buf).trim_end().to_string())
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
