//! Snapshot + element-ref types.
//!
//! ADR-028 invariants:
//! - Refs are versioned (`@v<N>:e<id>`). Old refs are stale after the next
//!   snapshot bumps the version.
//! - Snapshot is the only legal source of refs. Provider must reject any
//!   ref it didn't issue this version.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub struct SnapshotVersion(pub u64);

impl SnapshotVersion {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for SnapshotVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Opaque element ref. Wire format: `@v<N>:e<id>`.
///
/// Custom (de)serialise as the rendered string so JSON consumers see
/// `"@v1:e3"`, not a nested object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementRef {
    pub version: SnapshotVersion,
    pub element_id: String,
}

impl Serialize for ElementRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.render())
    }
}

impl<'de> Deserialize<'de> for ElementRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid element ref: {s}")))
    }
}

impl ElementRef {
    pub fn new(version: SnapshotVersion, element_id: impl Into<String>) -> Self {
        Self {
            version,
            element_id: element_id.into(),
        }
    }

    pub fn render(&self) -> String {
        format!("@v{}:e{}", self.version.0, self.element_id)
    }

    /// Parse `@v<N>:e<id>` form. Returns `None` on malformed input.
    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix("@v")?;
        let (vnum, eid) = rest.split_once(":e")?;
        let version = SnapshotVersion(vnum.parse().ok()?);
        Some(Self {
            version,
            element_id: eid.to_string(),
        })
    }
}

impl std::fmt::Display for ElementRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotElement {
    #[serde(rename = "ref")]
    pub r#ref: ElementRef,
    /// ARIA role or HTML semantic role. Examples: `button`, `textbox`, `link`,
    /// `heading`, `img`.
    pub role: String,
    /// Accessible name (button label, link text, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Selector hint for debug + audit. Not used by the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector_hint: Option<String>,
    /// Did the prompt-injection scanner flag this element?
    #[serde(default, skip_serializing_if = "is_false")]
    pub injection_flagged: bool,
    /// Whether the element accepts a click/type/etc. in its current state.
    #[serde(default = "default_actionable")]
    pub actionable: bool,
    /// `false` if `disabled` or `aria-disabled="true"`. Default `true` so
    /// older snapshot logs without this field deserialise correctly.
    #[serde(default = "default_actionable")]
    pub enabled: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn default_actionable() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub url: String,
    pub title: String,
    /// Per ADR-028: monotonic per session.
    pub version: SnapshotVersion,
    /// Total raw a11y nodes scanned (most filtered out — only actionable
    /// nodes appear in `elements`).
    pub raw_node_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub meta: SnapshotMeta,
    pub elements: Vec<SnapshotElement>,
}

impl Snapshot {
    pub fn lookup(&self, r: &ElementRef) -> Option<&SnapshotElement> {
        if r.version != self.meta.version {
            return None;
        }
        self.elements.iter().find(|e| e.r#ref == *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_roundtrip() {
        let r = ElementRef::new(SnapshotVersion(3), "12");
        let s = r.render();
        assert_eq!(s, "@v3:e12");
        let parsed = ElementRef::parse(&s).unwrap();
        assert_eq!(parsed, r);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(ElementRef::parse("foo").is_none());
        assert!(ElementRef::parse("@v3").is_none());
        assert!(ElementRef::parse("@vX:e12").is_none());
    }

    #[test]
    fn lookup_rejects_stale_version() {
        let snap = Snapshot {
            meta: SnapshotMeta {
                url: "about:blank".into(),
                title: "".into(),
                version: SnapshotVersion(2),
                raw_node_count: 0,
            },
            elements: vec![SnapshotElement {
                r#ref: ElementRef::new(SnapshotVersion(2), "1"),
                role: "button".into(),
                name: None,
                selector_hint: None,
                injection_flagged: false,
                actionable: true,
                enabled: true,
            }],
        };
        let stale = ElementRef::new(SnapshotVersion(1), "1");
        assert!(snap.lookup(&stale).is_none());
        let fresh = ElementRef::new(SnapshotVersion(2), "1");
        assert!(snap.lookup(&fresh).is_some());
    }
}
