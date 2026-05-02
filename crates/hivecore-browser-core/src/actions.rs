//! Action / wait / assert types — model-facing surface for `browser_act`,
//! `browser_wait`, `browser_assert`.
//!
//! ADR-028: `browser_act` is one tool with a typed `kind` enum (open
//! question #2 resolved in favour of unified tool to keep the tool list
//! compact). The kinds collapse 10+ primitives into one model-visible
//! verb space.

use serde::{Deserialize, Serialize};

use crate::snapshot::ElementRef;

/// What `browser_act` does to the targeted ref.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    Click,
    /// Append text. Does not clear; use `Fill` for replace-semantics.
    Type {
        text: String,
    },
    /// Replace existing value with `text`.
    Fill {
        text: String,
    },
    /// Send a single key (Enter, Tab, ArrowDown, ...).
    Press {
        key: String,
    },
    Hover,
    /// Toggle a checkbox / radio.
    SetChecked {
        checked: bool,
    },
    /// Pick a `<select>` option by visible label.
    Select {
        option: String,
    },
    /// Drag from `from` to `to` ref (both must be from current snapshot).
    Drag {
        to: ElementRef,
    },
    /// Local file path (must live under tenant artifact root).
    Upload {
        path: String,
    },
    /// Scroll element into view.
    Scroll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutcome {
    /// Did the underlying CDP/driver call succeed?
    pub ok: bool,
    /// Human-readable description of what happened. Goes to audit only;
    /// agent reads the post-action snapshot for state.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// Conditions for `browser_wait`. Fires until the condition holds or a
/// timeout fires. Mirrors gsd-browser's wait surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case")]
pub enum WaitCondition {
    Load,
    DomContentLoaded,
    NetworkIdle,
    SelectorVisible {
        selector: String,
    },
    SelectorHidden {
        selector: String,
    },
    RefVisible {
        r#ref: ElementRef,
    },
    UrlContains {
        substring: String,
    },
    UrlMatches {
        /// Regex. Compiled provider-side.
        pattern: String,
    },
    TextVisible {
        substring: String,
    },
    TextHidden {
        substring: String,
    },
    /// Sleep for N milliseconds. Last-resort use only.
    Delay {
        ms: u64,
    },
}

/// Assertion DSL for `browser_assert`. Records audit-grade pass/fail with
/// human-readable evidence in details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "predicate", rename_all = "snake_case")]
pub enum AssertPredicate {
    /// Element exists at this ref in current snapshot.
    Exists {
        r#ref: ElementRef,
    },
    /// Element's accessible-name equals `expected`.
    NameEquals {
        r#ref: ElementRef,
        expected: String,
    },
    UrlEquals {
        expected: String,
    },
    UrlContains {
        substring: String,
    },
    TextVisible {
        substring: String,
    },
    Count {
        role: String,
        expected: usize,
    },
}
