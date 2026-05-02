//! Capability schema — every provider declares which BPP methods it
//! actually supports, with a stability tier. Lifted from jcode's BPP doc
//! (MIT) — see `.planning/intel/browser-harness.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Capability stability classification. The model + harness can decide
/// whether to call experimental capabilities; `NotImplemented` means the
/// provider returns `BrowserError::CapabilityMissing` if invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStability {
    /// Spec-compliant, stable, and tested.
    Stable,
    /// Implemented but may have rough edges or breakages.
    Experimental,
    /// Provider does not implement this capability at all.
    NotImplemented,
}

/// Well-known capability identifiers. New providers can advertise
/// additional ones via the `extras` map on `CapabilityTable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Capability {
    /// `page.snapshot` returns versioned a11y refs.
    A11ySnapshot,
    /// Element refs are stable within a snapshot version.
    ElementRefs,
    /// `page.screenshot` works.
    Screenshot,
    /// Provider can attach to a user's existing browser process.
    AttachExistingBrowser,
    /// Provider keeps cookies / localStorage across sessions.
    PersistentProfile,
    /// Each session is a fresh isolated context.
    IsolatedContexts,
    /// `page.eval` is exposed (capability-gated; see ADR-028 §6).
    PageEval,
    /// Provider can return network requests for the current page.
    NetworkInspection,
    /// Provider can mock / block routes.
    NetworkRouting,
    /// Vision delegate is wired (`browser_scene_understand` /
    /// `browser_vision_click`).
    VisionFallback,
    /// Provider supports `BrowserHook` sensitive-action gating.
    SensitiveActionHooks,
    /// Per-tenant credential vault (AES-GCM + Argon2 keyed by `tenant_id`).
    CredentialVault,
    /// Always-on prompt-injection scan tags snapshots.
    InjectionScan,
    /// Provider supports the `--storage-state` cookie/storage migration
    /// shape (Playwright pattern).
    StorageState,
    /// Multiple pages within one session.
    MultiPage,
    /// Iframes addressable as frames.
    Frames,
    /// Recording / trace export (`har`, `trace`).
    Tracing,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityTable {
    pub provider_id: String,
    pub provider_version: String,
    pub capabilities: HashMap<Capability, CapabilityStability>,
    /// Provider-specific extensions (e.g. `cdp.invoke`, `firefox.install_extension`).
    /// Namespaced by provider id.
    #[serde(default)]
    pub extras: HashMap<String, CapabilityStability>,
}

impl CapabilityTable {
    pub fn supports(&self, c: Capability) -> bool {
        matches!(
            self.capabilities.get(&c),
            Some(CapabilityStability::Stable | CapabilityStability::Experimental)
        )
    }

    pub fn require(&self, c: Capability) -> Result<(), String> {
        if self.supports(c) {
            Ok(())
        } else {
            Err(format!(
                "provider '{}' does not support capability {:?}",
                self.provider_id, c
            ))
        }
    }
}
