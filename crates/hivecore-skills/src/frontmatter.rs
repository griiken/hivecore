//! Splits a markdown file into `(yaml_frontmatter, body)`. Tolerates files
//! with no frontmatter (returns `(None, whole_file)`).

use crate::error::SkillError;

/// Parse a markdown file's leading YAML frontmatter.
///
/// Convention follows Jekyll/Hugo/Claude-Code/Codex/GSD: file starts with
/// `---\n`, frontmatter, `---\n`, then body.
pub fn split<'a>(text: &'a str, path: &str) -> Result<(Option<&'a str>, &'a str), SkillError> {
    let trimmed = text.trim_start_matches('\u{FEFF}'); // strip BOM if present

    if !trimmed.starts_with("---\n") && !trimmed.starts_with("---\r\n") {
        return Ok((None, trimmed));
    }
    let after_open = trimmed
        .strip_prefix("---\n")
        .or_else(|| trimmed.strip_prefix("---\r\n"))
        .unwrap();

    // Find the closing `---` line.
    let mut close: Option<usize> = None;
    let mut idx = 0;
    for line in after_open.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches(['\n', '\r']);
        if trimmed_line == "---" {
            close = Some(idx);
            break;
        }
        idx += line.len();
    }
    let Some(close) = close else {
        return Err(SkillError::BadFrontmatter {
            path: path.to_string(),
            message: "frontmatter opened with `---` but never closed".into(),
        });
    };

    let yaml = &after_open[..close];
    let body_with_terminator = &after_open[close..];
    // Skip the closing `---` line + its newline.
    let body = body_with_terminator
        .strip_prefix("---\n")
        .or_else(|| body_with_terminator.strip_prefix("---\r\n"))
        .or_else(|| body_with_terminator.strip_prefix("---"))
        .unwrap_or(body_with_terminator);
    Ok((Some(yaml), body))
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
