//! `ChromeProvider` — chromiumoxide-backed `BrowserProvider` impl.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use hivecore_browser_core::{
    ActionKind, ActionOutcome, AssertPredicate, BrowserContext, BrowserError, BrowserProvider,
    BrowserResult, BrowserSessionId, Capability, CapabilityStability, CapabilityTable, ElementRef,
    Snapshot, SnapshotElement, SnapshotMeta, SnapshotVersion, TenantId, WaitCondition,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ChromeConfig {
    /// Path to the Chrome / Chromium binary. Defaults to
    /// `/usr/bin/google-chrome-stable`.
    pub chrome_path: Option<String>,
    /// Run headless (default: true).
    pub headless: bool,
    /// Disable Chrome's own sandbox. Required in many CI environments.
    pub no_sandbox: bool,
    /// Tenant-scoped artifact root. Per ADR-028 §"Safety": all
    /// screenshots / traces / downloads land under
    /// `<artifact_root>/<tenant_id>/<session_id>/`.
    pub artifact_root: PathBuf,
}

impl Default for ChromeConfig {
    fn default() -> Self {
        let mut artifact_root = std::env::var("HIVECORE_SESSIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".hivecore").join("sessions")
            });
        artifact_root.push("__browser__");
        Self {
            chrome_path: Some("/usr/bin/google-chrome-stable".to_string()),
            headless: true,
            no_sandbox: true,
            artifact_root,
        }
    }
}

/// Per-session state owned by the provider. Each session = one Chrome
/// browser process + one default page + a monotonic snapshot version.
struct Session {
    tenant: TenantId,
    browser: Browser,
    page: Page,
    /// Drives the chromiumoxide event loop until session shutdown.
    handler_task: tokio::task::JoinHandle<()>,
    snapshot_version: SnapshotVersion,
    /// `element_id` → `enabled` for the latest snapshot. Used by `act()`
    /// for stale-ref check (key absent ⇒ id not in current snapshot)
    /// and disabled-element guard (value `false` ⇒ NotActionable).
    last_snapshot_elements: std::collections::HashMap<String, bool>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("tenant", &self.tenant)
            .field("snapshot_version", &self.snapshot_version)
            .finish()
    }
}

#[derive(Debug)]
pub struct ChromeProvider {
    config: ChromeConfig,
    capabilities: CapabilityTable,
    sessions: Arc<Mutex<HashMap<BrowserSessionId, Session>>>,
}

impl ChromeProvider {
    pub fn new(config: ChromeConfig) -> Self {
        let mut caps = CapabilityTable {
            provider_id: "chromiumoxide".into(),
            provider_version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        };
        let stable = CapabilityStability::Stable;
        caps.capabilities.insert(Capability::A11ySnapshot, stable);
        caps.capabilities.insert(Capability::ElementRefs, stable);
        caps.capabilities.insert(Capability::Screenshot, stable);
        caps.capabilities
            .insert(Capability::IsolatedContexts, stable);
        caps.capabilities.insert(Capability::MultiPage, stable);
        Self {
            config,
            capabilities: caps,
            sessions: Default::default(),
        }
    }

    fn build_browser_config(&self) -> Result<BrowserConfig, String> {
        let mut b = BrowserConfig::builder();
        if !self.config.headless {
            b = b.with_head();
        }
        if let Some(path) = &self.config.chrome_path {
            b = b.chrome_executable(path);
        }
        if self.config.no_sandbox {
            b = b.arg("--no-sandbox");
        }
        b.build()
    }

    /// Verify a session belongs to the requesting tenant. ADR-028 multi-
    /// tenant invariant — provider rejects cross-tenant access by
    /// construction.
    fn verify_tenant(&self, session: &Session, ctx: &BrowserContext) -> BrowserResult<()> {
        if session.tenant != ctx.tenant {
            return Err(BrowserError::CrossTenantDenied {
                caller: ctx.tenant.0.clone(),
                owner: session.tenant.0.clone(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl BrowserProvider for ChromeProvider {
    fn id(&self) -> &str {
        "chromiumoxide"
    }

    fn capabilities(&self) -> &CapabilityTable {
        &self.capabilities
    }

    async fn ensure_session(
        &self,
        tenant: &TenantId,
        spec: hivecore_browser_core::provider::SessionSpec,
    ) -> BrowserResult<BrowserSessionId> {
        let cfg = self
            .build_browser_config()
            .map_err(BrowserError::Transport)?;
        let (browser, mut handler) = Browser::launch(cfg)
            .await
            .map_err(|e| BrowserError::Transport(e.to_string()))?;
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if let Err(e) = h {
                    tracing::debug!(error = %e, "chromiumoxide handler error (likely shutdown)");
                }
            }
        });

        let initial = spec.initial_url.as_deref().unwrap_or("about:blank");
        let page = browser
            .new_page(initial)
            .await
            .map_err(|e| BrowserError::Transport(e.to_string()))?;
        if spec.initial_url.is_some() {
            let _ = page.wait_for_navigation().await;
        }

        let session_id = BrowserSessionId::new(
            spec.session_name_hint
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        );
        let mut sessions = self.sessions.lock().await;
        sessions.insert(
            session_id.clone(),
            Session {
                tenant: tenant.clone(),
                browser,
                page,
                handler_task,
                snapshot_version: SnapshotVersion::default(),
                last_snapshot_elements: std::collections::HashMap::new(),
            },
        );
        Ok(session_id)
    }

    async fn close_session(&self, ctx: &BrowserContext) -> BrowserResult<()> {
        let mut sessions = self.sessions.lock().await;
        let Some(mut session) = sessions.remove(&ctx.session) else {
            return Ok(());
        };
        if session.tenant != ctx.tenant {
            let owner = session.tenant.0.clone();
            sessions.insert(ctx.session.clone(), session);
            return Err(BrowserError::CrossTenantDenied {
                caller: ctx.tenant.0.clone(),
                owner,
            });
        }
        let _ = session.browser.close().await;
        session.handler_task.abort();
        Ok(())
    }

    async fn list_sessions(&self, tenant: &TenantId) -> BrowserResult<Vec<BrowserSessionId>> {
        let sessions = self.sessions.lock().await;
        Ok(sessions
            .iter()
            .filter(|(_, s)| s.tenant == *tenant)
            .map(|(id, _)| id.clone())
            .collect())
    }

    async fn navigate(&self, ctx: &BrowserContext, url: &str) -> BrowserResult<()> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&ctx.session)
            .ok_or_else(|| BrowserError::SessionNotFound(ctx.session.0.clone()))?;
        self.verify_tenant(session, ctx)?;
        session
            .page
            .goto(url)
            .await
            .map_err(|e| BrowserError::Navigation(e.to_string()))?;
        let _ = session.page.wait_for_navigation().await;
        Ok(())
    }

    async fn snapshot(&self, ctx: &BrowserContext) -> BrowserResult<Snapshot> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&ctx.session)
            .ok_or_else(|| BrowserError::SessionNotFound(ctx.session.0.clone()))?;
        self.verify_tenant(session, ctx)?;

        let new_version = session.snapshot_version.next();
        session.snapshot_version = new_version;

        // JS-based DOM walker. Stamps each actionable element with a
        // `data-hcr-ref` attribute keyed by the snapshot's monotonic
        // version + a sequential element id. This gives us a stable
        // CSS-selector bridge for `act()` (`[data-hcr-ref="@v3:e12"]`)
        // and is the same pattern gsd-browser uses.
        //
        // The walker also extracts ARIA role + accessible-name for the
        // model-facing snapshot. Returns a JSON array of {ref, role,
        // name, raw_count, url, title}.
        // JS DOM walker. Per ADR-028 + validation report fixes:
        //  - shadow-DOM aware (collectQueryRoots recurses ShadowRoot).
        //  - same-origin iframe enumeration (cross-origin reported but
        //    not entered).
        //  - dropped `heading` and `img` from actionable roles (informational).
        //  - added <summary>, <details>, <label>, contenteditable.
        //  - stricter visibility check (gsd-browser shape).
        //  - hard `limit` (default 200, configurable later via SnapshotMode).
        let script = format!(
            r#"
            (() => {{
                const VERSION = {version};
                const PREV_ATTR = 'data-hivecore-ref';
                const LIMIT = 200;
                const ACTIONABLE_ROLES = new Set([
                    'button','textbox','link','checkbox','combobox','menuitem',
                    'menuitemcheckbox','menuitemradio','option','radio',
                    'searchbox','tab','switch','slider','spinbutton','treeitem'
                    // Removed `gridcell`, `columnheader`, `rowheader`, `tabpanel`:
                    // gsd-browser doesn't include them; they pollute snapshots
                    // on data tables and tabbed UIs with non-actionable refs.
                ]);
                const TAG_TO_ROLE = {{
                    'BUTTON':'button','A':'link','SELECT':'combobox','TEXTAREA':'textbox',
                    'SUMMARY':'button','DETAILS':'button','LABEL':'button'
                }};
                function inferRole(el) {{
                    const explicit = el.getAttribute && el.getAttribute('role');
                    if (explicit && ACTIONABLE_ROLES.has(explicit)) return explicit;
                    if (el.isContentEditable) return 'textbox';
                    if (el.tagName === 'INPUT') {{
                        const t = (el.getAttribute('type') || 'text').toLowerCase();
                        if (t === 'checkbox') return 'checkbox';
                        if (t === 'radio') return 'radio';
                        if (t === 'submit' || t === 'button' || t === 'reset') return 'button';
                        if (t === 'hidden') return null;
                        return 'textbox';
                    }}
                    if (el.tagName === 'A' && !el.hasAttribute('href') && el.getAttribute('role') !== 'button') {{
                        // Anchor without href + no explicit role → not navigable. Skip.
                        return null;
                    }}
                    return TAG_TO_ROLE[el.tagName] || null;
                }}
                function accessibleName(el) {{
                    const aria = el.getAttribute && el.getAttribute('aria-label');
                    if (aria) return aria.trim();
                    const labelledBy = el.getAttribute && el.getAttribute('aria-labelledby');
                    if (labelledBy) {{
                        const labels = labelledBy.split(/\s+/).map(id => document.getElementById(id)).filter(Boolean);
                        const t = labels.map(l => (l.textContent || '').trim()).filter(Boolean).join(' ');
                        if (t) return t;
                    }}
                    if (el.tagName === 'INPUT') {{
                        const ph = el.getAttribute('placeholder');
                        if (ph) return ph;
                        if (el.id) {{
                            const lbl = document.querySelector('label[for="' + el.id + '"]');
                            if (lbl) return (lbl.textContent || '').trim();
                        }}
                    }}
                    if (el.tagName === 'IMG') return el.getAttribute('alt') || '';
                    const txt = (el.textContent || '').trim();
                    if (txt) return txt.length > 200 ? txt.slice(0, 200) : txt;
                    return '';
                }}
                function isVisible(el) {{
                    if (!el || !el.getBoundingClientRect) return false;
                    if (el.hasAttribute && el.hasAttribute('hidden')) return false;
                    const cs = window.getComputedStyle(el);
                    if (!cs) return true;
                    if (cs.display === 'none') return false;
                    if (cs.visibility === 'hidden' || cs.visibility === 'collapse') return false;
                    if (cs.opacity === '0') return false;
                    const r = el.getBoundingClientRect();
                    return r.width > 0 && r.height > 0;
                }}
                function isEnabled(el) {{
                    if (!el) return true;
                    if (el.disabled) return false;
                    if (el.getAttribute && el.getAttribute('aria-disabled') === 'true') return false;
                    return true;
                }}
                // Recurse into shadowRoot trees.
                function collectQueryRoots(scope, out) {{
                    out.push(scope);
                    const all = scope.querySelectorAll ? scope.querySelectorAll('*') : [];
                    for (const el of all) {{
                        if (el.shadowRoot) {{
                            collectQueryRoots(el.shadowRoot, out);
                        }}
                    }}
                }}
                // Iframe enumeration — for v0.1 we ONLY report frames we
                // see; we DO NOT walk into them, because chromiumoxide's
                // top-level `Page::find_element` cannot traverse iframe
                // contentDocument. Stamping refs inside frames would
                // produce unactionable refs (act would StaleRef).
                // v0.2: route per-frame via CDP `Target.attachToTarget`.
                function collectFrames() {{
                    const cross = [];
                    const iframes = document.querySelectorAll('iframe,frame');
                    for (const iframe of iframes) {{
                        cross.push(iframe.src || iframe.name || '(unnamed)');
                    }}
                    return {{ frames: [], cross }};
                }}
                // Clear stale stamps from prior versions (main doc + same-origin frames).
                document.querySelectorAll('[' + PREV_ATTR + ']').forEach(el => el.removeAttribute(PREV_ATTR));
                const elements = [];
                let counter = 0;
                let truncated = false;
                let raw_count = 0;
                const cross_origin_frames = [];
                function walk(rootDoc) {{
                    const roots = [];
                    collectQueryRoots(rootDoc, roots);
                    for (const scope of roots) {{
                        if (!scope.querySelectorAll) continue;
                        const candidates = scope.querySelectorAll('*');
                        raw_count += candidates.length;
                        for (const el of candidates) {{
                            if (counter >= LIMIT) {{ truncated = true; return; }}
                            const role = inferRole(el);
                            if (!role) continue;
                            if (!isVisible(el)) continue;
                            counter += 1;
                            const elementId = String(counter);
                            const ref = '@v' + VERSION + ':e' + elementId;
                            try {{ el.setAttribute(PREV_ATTR, ref); }} catch (e) {{}}
                            elements.push({{
                                ref: ref,
                                role: role,
                                name: accessibleName(el),
                                element_id: elementId,
                                enabled: isEnabled(el),
                            }});
                        }}
                    }}
                }}
                walk(document);
                const f = collectFrames();
                for (const {{ doc }} of f.frames) {{
                    if (counter >= LIMIT) {{ truncated = true; break; }}
                    walk(doc);
                }}
                cross_origin_frames.push(...f.cross);
                return JSON.stringify({{
                    elements: elements,
                    raw_count: raw_count,
                    url: document.location.href,
                    title: document.title,
                    truncated: truncated,
                    cross_origin_frames: cross_origin_frames,
                }});
            }})()
            "#,
            version = new_version.0,
        );

        let raw: serde_json::Value = session
            .page
            .evaluate(script.as_str())
            .await
            .map_err(|e| BrowserError::Transport(e.to_string()))?
            .into_value()
            .map_err(|e| BrowserError::Other(format!("evaluate decode: {e}")))?;
        // The JS returns a JSON string; parse it.
        let parsed: serde_json::Value = match raw {
            serde_json::Value::String(s) => serde_json::from_str(&s)
                .map_err(|e| BrowserError::Other(format!("snapshot json: {e}")))?,
            other => other,
        };

        let raw_count = parsed
            .get("raw_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let url = parsed
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = parsed
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let empty: Vec<serde_json::Value> = Vec::new();
        let js_elements = parsed
            .get("elements")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);

        let mut elements = Vec::new();
        let mut tracked: std::collections::HashMap<String, bool> = Default::default();
        for je in js_elements {
            let element_id = je
                .get("element_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if element_id.is_empty() {
                continue;
            }
            let role = je
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = je
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let enabled = je.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            let er = ElementRef::new(new_version, element_id.clone());
            tracked.insert(element_id, enabled);
            elements.push(SnapshotElement {
                r#ref: er,
                role,
                name,
                selector_hint: None,
                injection_flagged: false,
                actionable: enabled,
                enabled,
            });
        }
        session.last_snapshot_elements = tracked;

        Ok(Snapshot {
            meta: SnapshotMeta {
                url,
                title,
                version: new_version,
                raw_node_count: raw_count,
            },
            elements,
        })
    }

    async fn act(
        &self,
        ctx: &BrowserContext,
        target: &ElementRef,
        kind: ActionKind,
    ) -> BrowserResult<(ActionOutcome, Snapshot)> {
        // Resolve the page handle and stale-ref-check while holding the
        // session lock; clone the page handle so we can release the lock
        // before running async CDP calls.
        let page = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(&ctx.session)
                .ok_or_else(|| BrowserError::SessionNotFound(ctx.session.0.clone()))?;
            self.verify_tenant(session, ctx)?;
            if target.version != session.snapshot_version {
                return Err(BrowserError::StaleRef(target.render()));
            }
            // Validation report fix #8: "id not in snapshot" → StaleRef
            // (was NotActionable). Semantics: missing id means snapshot
            // is out-of-sync, not that the element exists-but-disabled.
            // Postfix-validation: real disabled-element guard now backed
            // by `last_snapshot_elements: HashMap<id, enabled>`.
            match session.last_snapshot_elements.get(&target.element_id) {
                None => return Err(BrowserError::StaleRef(target.render())),
                Some(false) => {
                    return Err(BrowserError::NotActionable(format!(
                        "{} (disabled in current snapshot)",
                        target.render()
                    )))
                }
                Some(true) => {}
            }
            session.page.clone()
        };

        // Validation report fix #6: wrap `find_element` in 5s timeout
        // (chromiumoxide #292 — hangs on invisible/animating elements).
        let selector = format!("[data-hivecore-ref=\"{}\"]", target.render());
        let element = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            page.find_element(selector.clone()),
        )
        .await
        {
            Ok(Ok(el)) => el,
            Ok(Err(e)) => return Err(BrowserError::StaleRef(format!("{}: {e}", target.render()))),
            Err(_) => {
                return Err(BrowserError::StaleRef(format!(
                    "{}: find_element timed out after 5s (element likely invisible or removed)",
                    target.render()
                )))
            }
        };

        // Helper: run a JS function on `this` (the element) — the only way
        // we get `el.click()` to fire React-compatible events without
        // tripping chromiumoxide #320.
        async fn js_call(element: &chromiumoxide::Element, decl: &str) -> BrowserResult<()> {
            element
                .call_js_fn(decl, false)
                .await
                .map_err(|e| BrowserError::Other(e.to_string()))?;
            Ok(())
        }

        let note: String;
        match &kind {
            // Validation report fix #1: replace `Element::click()` with
            // JS-eval click to dodge chromiumoxide #320 hang.
            ActionKind::Click => {
                element
                    .scroll_into_view()
                    .await
                    .map_err(|e| BrowserError::Other(e.to_string()))?;
                js_call(
                    &element,
                    "function() { try { this.focus({preventScroll:true}); } catch(e) {} this.click(); }",
                )
                .await?;
                note = format!("click {}", target.render());
            }
            // Validation report fix #2: type via JS so React's _valueTracker
            // sees the change (CDP `Input.insertText` bypasses it).
            ActionKind::Type { text } => {
                let v = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
                let js = format!(
                    r#"function() {{
                        const v = {v};
                        if (this.isContentEditable) {{
                            this.focus(); this.textContent = (this.textContent || '') + v;
                            this.dispatchEvent(new Event('input', {{bubbles:true}}));
                            return;
                        }}
                        if ('value' in this) {{
                            const proto = Object.getPrototypeOf(this);
                            const setter = Object.getOwnPropertyDescriptor(proto, 'value') &&
                                           Object.getOwnPropertyDescriptor(proto, 'value').set;
                            this.focus();
                            const next = (this.value || '') + v;
                            if (setter) setter.call(this, next); else this.value = next;
                            this.dispatchEvent(new Event('input', {{bubbles:true}}));
                            this.dispatchEvent(new Event('change', {{bubbles:true}}));
                        }}
                    }}"#
                );
                js_call(&element, &js).await?;
                note = format!("type {} chars into {}", text.len(), target.render());
            }
            ActionKind::Fill { text } => {
                let v = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
                // Replace-semantics: clear then set + dispatch input + change.
                // Uses the prototype's value setter so React's _valueTracker
                // is notified (gsd-browser pattern, validation report fix #2).
                let js = format!(
                    r#"function() {{
                        const v = {v};
                        if (this.isContentEditable) {{
                            this.focus(); this.textContent = v;
                            this.dispatchEvent(new Event('input', {{bubbles:true}}));
                            return;
                        }}
                        if ('value' in this) {{
                            const proto = Object.getPrototypeOf(this);
                            const setter = Object.getOwnPropertyDescriptor(proto, 'value') &&
                                           Object.getOwnPropertyDescriptor(proto, 'value').set;
                            this.focus();
                            if (setter) setter.call(this, v); else this.value = v;
                            this.dispatchEvent(new Event('input', {{bubbles:true}}));
                            this.dispatchEvent(new Event('change', {{bubbles:true}}));
                        }}
                    }}"#
                );
                js_call(&element, &js).await?;
                note = format!("fill {} chars into {}", text.len(), target.render());
            }
            ActionKind::Press { key } => {
                element
                    .press_key(key)
                    .await
                    .map_err(|e| BrowserError::Other(e.to_string()))?;
                note = format!("press {key}");
            }
            ActionKind::Hover => {
                element
                    .hover()
                    .await
                    .map_err(|e| BrowserError::Other(e.to_string()))?;
                note = format!("hover {}", target.render());
            }
            ActionKind::SetChecked { checked } => {
                let want = *checked;
                let js = format!(
                    r#"function() {{
                        const want = {want};
                        if (this.checked !== want) {{
                            this.click();
                            this.dispatchEvent(new Event('change', {{bubbles:true}}));
                        }}
                    }}"#
                );
                js_call(&element, &js).await?;
                note = format!("set_checked={want}");
            }
            ActionKind::Select { option } => {
                let label = serde_json::to_string(option).unwrap_or_else(|_| "\"\"".into());
                let js = format!(
                    r#"function() {{
                        const label = {label};
                        function norm(s) {{ return (s || '').replace(/\s+/g, ' ').trim(); }}
                        for (const o of this.options) {{
                            if (o.label === label || o.text === label || o.value === label
                                || norm(o.textContent) === label) {{
                                this.value = o.value;
                                this.dispatchEvent(new Event('change', {{bubbles:true}}));
                                return true;
                            }}
                        }}
                        return false;
                    }}"#
                );
                js_call(&element, &js).await?;
                note = format!("select {option}");
            }
            ActionKind::Scroll => {
                element
                    .scroll_into_view()
                    .await
                    .map_err(|e| BrowserError::Other(e.to_string()))?;
                note = format!("scroll {}", target.render());
            }
            ActionKind::Drag { to } => {
                tracing::warn!(?to, "drag not implemented in v0.1 — treating as no-op");
                note = format!("[stub] drag {} → {}", target.render(), to.render());
            }
            ActionKind::Upload { path } => {
                tracing::warn!(?path, "upload not implemented in v0.1 — treating as no-op");
                note = format!("[stub] upload {path}");
            }
        }

        // Settle loop — wait for the DOM to quiesce before re-snapshotting
        // so the agent's next turn sees a stable state. Replaces the prior
        // 50 ms placeholder per validation report. Adapted from gsd-browser
        // (Apache-2.0+MIT, see `settle.rs` header).
        let settle =
            crate::settle::settle_after_action(&page, &crate::settle::SettleOptions::default())
                .await;
        tracing::debug!(
            ?settle,
            target = %target,
            "post-action settle complete"
        );

        let snap = self.snapshot(ctx).await?;
        let mut note = note;
        note.push_str(&format!(
            " (settle: {} in {} ms, {} mutations)",
            settle.settle_reason, settle.settle_ms, settle.total_mutations
        ));
        Ok((ActionOutcome { ok: true, note }, snap))
    }

    async fn wait_for(
        &self,
        ctx: &BrowserContext,
        condition: WaitCondition,
        timeout_ms: u64,
    ) -> BrowserResult<()> {
        // Resolve page handle + tenant check + (for RefVisible) ref→selector
        // bridge under the lock; release before polling so concurrent ops
        // can proceed.
        let page = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(&ctx.session)
                .ok_or_else(|| BrowserError::SessionNotFound(ctx.session.0.clone()))?;
            self.verify_tenant(session, ctx)?;
            session.page.clone()
        };
        const POLL_INTERVAL_MS: u64 = 100;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        match condition {
            WaitCondition::Delay { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                Ok(())
            }
            WaitCondition::Load | WaitCondition::DomContentLoaded => {
                tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    page.wait_for_navigation(),
                )
                .await
                .map_err(|_| BrowserError::Other(format!("wait timeout {timeout_ms}ms")))?
                .map_err(|e| BrowserError::Transport(e.to_string()))?;
                Ok(())
            }
            WaitCondition::SelectorVisible { selector } => {
                wait_selector(&page, &selector, true, deadline, POLL_INTERVAL_MS).await
            }
            WaitCondition::SelectorHidden { selector } => {
                wait_selector(&page, &selector, false, deadline, POLL_INTERVAL_MS).await
            }
            WaitCondition::RefVisible { r#ref } => {
                // Bridge ref → CSS selector via the stamped attribute.
                let selector = format!("[data-hivecore-ref=\"{}\"]", r#ref.render());
                wait_selector(&page, &selector, true, deadline, POLL_INTERVAL_MS).await
            }
            WaitCondition::UrlContains { substring } => {
                wait_url(&page, deadline, POLL_INTERVAL_MS, |url| {
                    url.contains(&substring)
                })
                .await
            }
            WaitCondition::UrlMatches { pattern } => {
                let re = regex::Regex::new(&pattern).map_err(|e| {
                    BrowserError::Other(format!("invalid url regex `{pattern}`: {e}"))
                })?;
                wait_url(&page, deadline, POLL_INTERVAL_MS, |url| re.is_match(url)).await
            }
            WaitCondition::TextVisible { substring } => {
                wait_text(&page, &substring, true, deadline, POLL_INTERVAL_MS).await
            }
            WaitCondition::TextHidden { substring } => {
                wait_text(&page, &substring, false, deadline, POLL_INTERVAL_MS).await
            }
            WaitCondition::NetworkIdle => {
                // v0.1: JS-side fallback — count in-flight fetches via a
                // performance-entry tail. gsd-browser uses a daemon-side
                // ring buffer (faster, more accurate); we'll add that with
                // CDP `Network.requestWillBeSent` in v0.2.
                wait_network_idle(&page, deadline, POLL_INTERVAL_MS, 500).await
            }
        }
    }

    async fn assert(
        &self,
        ctx: &BrowserContext,
        predicate: AssertPredicate,
    ) -> BrowserResult<bool> {
        let snap = self.snapshot(ctx).await?;
        let result = match predicate {
            AssertPredicate::Exists { r#ref } => snap.lookup(&r#ref).is_some(),
            AssertPredicate::NameEquals { r#ref, expected } => snap
                .lookup(&r#ref)
                .and_then(|e| e.name.as_deref())
                .map(|n| n == expected)
                .unwrap_or(false),
            AssertPredicate::UrlEquals { expected } => snap.meta.url == expected,
            AssertPredicate::UrlContains { substring } => snap.meta.url.contains(&substring),
            AssertPredicate::TextVisible { substring } => snap
                .elements
                .iter()
                .filter_map(|e| e.name.as_deref())
                .any(|n| n.contains(&substring)),
            AssertPredicate::Count { role, expected } => {
                snap.elements.iter().filter(|e| e.role == role).count() == expected
            }
        };
        Ok(result)
    }

    async fn screenshot(&self, ctx: &BrowserContext) -> BrowserResult<Vec<u8>> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(&ctx.session)
            .ok_or_else(|| BrowserError::SessionNotFound(ctx.session.0.clone()))?;
        self.verify_tenant(session, ctx)?;
        let bytes = session
            .page
            .screenshot(CaptureScreenshotParams::default())
            .await
            .map_err(|e| BrowserError::Transport(e.to_string()))?;
        Ok(bytes)
    }
}

// ===== WaitCondition helpers (Concern B v0.2) ===========================
//
// gsd-browser's `poll_until` shape: poll a JS evaluator at fixed cadence
// until the condition holds or the deadline passes. On timeout we return
// `BrowserError::Other("wait timeout: ...")` rather than gsd's `met:false`
// — our trait surface returns `BrowserResult<()>`, so timeout is an Err.
// Callers wanting `met:false` semantics can `.ok()` on the result.

async fn poll_until<F, Fut>(
    deadline: std::time::Instant,
    interval_ms: u64,
    mut check: F,
    label: &str,
) -> BrowserResult<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    loop {
        if check().await {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(BrowserError::Other(format!("wait timeout: {label}")));
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
    }
}

/// Eval a JS expression that returns a JSON-stringified value; coerce to
/// `serde_json::Value`. Returns `Value::Null` on any error so the poll can
/// keep trying.
async fn eval_json(page: &chromiumoxide::Page, script: &str) -> serde_json::Value {
    let evaluated = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        page.evaluate_expression(script),
    )
    .await
    {
        Ok(Ok(v)) => v,
        _ => return serde_json::Value::Null,
    };
    let raw: serde_json::Value = match evaluated.into_value() {
        Ok(v) => v,
        Err(_) => return serde_json::Value::Null,
    };
    match raw {
        serde_json::Value::String(s) => {
            serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
        }
        other => other,
    }
}

async fn wait_selector(
    page: &chromiumoxide::Page,
    selector: &str,
    want_visible: bool,
    deadline: std::time::Instant,
    interval_ms: u64,
) -> BrowserResult<()> {
    // JS evaluator: returns `{count, visible}`. "Visible" mirrors gsd-browser
    // (display!=none && visibility!=hidden && bounding rect > 0).
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".into());
    let script = format!(
        r#"
        (() => {{
            const sel = {sel_json};
            try {{
                const el = document.querySelector(sel);
                if (!el) return JSON.stringify({{ count: 0, visible: false }});
                const cs = window.getComputedStyle(el);
                if (cs.display === 'none' || cs.visibility === 'hidden') {{
                    return JSON.stringify({{ count: 1, visible: false }});
                }}
                const r = el.getBoundingClientRect();
                return JSON.stringify({{ count: 1, visible: r.width > 0 && r.height > 0 }});
            }} catch (e) {{
                return JSON.stringify({{ count: 0, visible: false, err: String(e) }});
            }}
        }})()
        "#
    );
    poll_until(
        deadline,
        interval_ms,
        || async {
            let v = eval_json(page, &script).await;
            let count = v.get("count").and_then(|x| x.as_u64()).unwrap_or(0);
            let visible = v.get("visible").and_then(|x| x.as_bool()).unwrap_or(false);
            if want_visible {
                count > 0 && visible
            } else {
                count == 0 || !visible
            }
        },
        if want_visible {
            "selector_visible"
        } else {
            "selector_hidden"
        },
    )
    .await
}

async fn wait_url<F>(
    page: &chromiumoxide::Page,
    deadline: std::time::Instant,
    interval_ms: u64,
    matcher: F,
) -> BrowserResult<()>
where
    F: Fn(&str) -> bool,
{
    poll_until(
        deadline,
        interval_ms,
        || async {
            let url = page.url().await.ok().flatten().unwrap_or_default();
            matcher(&url)
        },
        "url",
    )
    .await
}

async fn wait_text(
    page: &chromiumoxide::Page,
    substring: &str,
    want_visible: bool,
    deadline: std::time::Instant,
    interval_ms: u64,
) -> BrowserResult<()> {
    // v0.1: substring match against `document.body.innerText`. Promote
    // to a TreeWalker(NodeFilter.SHOW_TEXT) impl if false negatives bite.
    let needle_json = serde_json::to_string(substring).unwrap_or_else(|_| "\"\"".into());
    let script = format!(
        r#"
        (() => {{
            const needle = {needle_json};
            try {{
                const t = (document.body && document.body.innerText) || '';
                return JSON.stringify({{ found: t.indexOf(needle) >= 0 }});
            }} catch (e) {{
                return JSON.stringify({{ found: false, err: String(e) }});
            }}
        }})()
        "#
    );
    poll_until(
        deadline,
        interval_ms,
        || async {
            let v = eval_json(page, &script).await;
            let found = v.get("found").and_then(|x| x.as_bool()).unwrap_or(false);
            if want_visible {
                found
            } else {
                !found
            }
        },
        if want_visible {
            "text_visible"
        } else {
            "text_hidden"
        },
    )
    .await
}

async fn wait_network_idle(
    page: &chromiumoxide::Page,
    deadline: std::time::Instant,
    interval_ms: u64,
    idle_window_ms: u64,
) -> BrowserResult<()> {
    // JS-side fallback: page-side `PerformanceObserver` for `resource`
    // entries. Our daemon doesn't yet keep a network ring buffer (gsd-browser
    // pattern); v0.2 will route via CDP `Network.requestWillBeSent` and
    // remove this approximation. For now: if no new resource entries
    // arrive in `idle_window_ms`, declare idle.
    let install = r#"
        (() => {
            if (window.__hivecoreNetIdleInstalled) return true;
            try {
                window.__hivecoreNetIdleLastEntry = Date.now();
                window.__hivecoreNetIdleCount = 0;
                const po = new PerformanceObserver((list) => {
                    for (const _ of list.getEntries()) {
                        window.__hivecoreNetIdleLastEntry = Date.now();
                        window.__hivecoreNetIdleCount += 1;
                    }
                });
                po.observe({ type: 'resource', buffered: true });
                window.__hivecoreNetIdleInstalled = true;
                return true;
            } catch (e) { return false; }
        })()
    "#;
    let _ = eval_json(page, install).await;
    let read = r#"
        (() => {
            const last = window.__hivecoreNetIdleLastEntry || 0;
            const elapsed = Date.now() - last;
            return JSON.stringify({ idle_ms: elapsed });
        })()
    "#;
    poll_until(
        deadline,
        interval_ms,
        || async {
            let v = eval_json(page, read).await;
            let idle_ms = v.get("idle_ms").and_then(|x| x.as_u64()).unwrap_or(0);
            idle_ms >= idle_window_ms
        },
        "network_idle",
    )
    .await
}
