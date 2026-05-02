# Browser harness validation — implementation vs OSS prior art

## Methodology

Date: 2026-05-01.

Read in full:
- `crates/hivecore-browser-core/src/{snapshot,actions,provider}.rs`
- `crates/hivecore-browser-runtime/src/chrome_provider.rs`
- `crates/hivecore-browser-harness/src/tools.rs`

Compared against (fetched fresh via `gh api`):
- `gsd-build/gsd-browser` @ main:
  - `cli/src/daemon/handlers/refs.rs` (465 LOC)
  - `cli/src/daemon/inspection.rs` (1287 LOC) — load-bearing JS walker + ref resolver
  - `cli/src/daemon/settle.rs` (247 LOC) — MutationObserver settle loop
  - `cli/src/daemon/handlers/{interaction,forms,wait,assert_cmd}.rs`
- `microsoft/playwright-mcp` @ main: README tool listing, `index.js` (delegates to playwright-core internal). Source of truth for tool *names*.
- `1jehuang/jcode` @ main: `src/tool/browser.rs` — Firefox-bridge, not a chromium impl. Limited comparison value.
- `chromiumoxide` issues 292, 320, 296, 285, 320 (open bugs in our SDK).

URLs:
- https://github.com/gsd-build/gsd-browser/blob/main/cli/src/daemon/inspection.rs
- https://github.com/gsd-build/gsd-browser/blob/main/cli/src/daemon/handlers/refs.rs
- https://github.com/microsoft/playwright-mcp/blob/main/README.md
- https://github.com/mattsse/chromiumoxide/issues/320
- https://github.com/mattsse/chromiumoxide/issues/292

## gsd-browser comparison

### Snapshot walker

**Theirs** (`inspection.rs:837-979`, `snapshot_elements`):

```js
const resolved = collectContexts(frameSpec, true);   // multi-frame
// for each accessible same-origin frame:
for (const scope of scopes) {
  const queried = queryAllDeep(scope, null);          // walks shadow DOM
  for (const el of queried.elements) {
    if (!includeElement(el, isVisible(el))) continue;
    results.push({
      tag, role, name,
      x, y, w, h,                                    // absolute bounds (frame-corrected)
      selectorHints: selectorHints(el),              // multi-tier locator
      visible, enabled,
      deepPath: deepDomPath(el),                     // shadow-aware path
      contentHash: simpleHash(`${tag}:${text}`),     // fingerprint tier 4
      structuralSignature: `${tag}:${childCount}:${attrCount}`,
      nearestHeading: nearestHeading(el),            // contextual label
      formOwnership: formOwnership(el),
      frameIndex, frameName, frameLabel, frameUrl,
    });
  }
}
```

**Ours** (`chrome_provider.rs:249-330`):

```js
const all = document.querySelectorAll('*');
for (const el of all) {
  const role = inferRole(el);
  if (!role) continue;
  if (!visible(el)) continue;
  el.setAttribute('data-hcr-ref', '@v'+VERSION+':e'+counter);
  elements.push({ ref, role, name, element_id });
}
```

Differences:

| Aspect | gsd-browser | ours |
|---|---|---|
| Frame walk | yes (same-origin recurse, cross-origin boundary report) | **no** — main frame only |
| Shadow DOM | yes (`collectQueryRoots` recurses `shadowRoot`) | **no** — `querySelectorAll('*')` skips shadow |
| Bounds | absolute `x,y,w,h` (frame-corrected) | none |
| Visibility | `display:none` + `visibility:hidden` + `width>0 && height>0` | only `display:none`/`visibility:hidden` *if* `width===0 && height===0` — looser, lets some hidden elements through |
| Stamping | does **not** mutate DOM. Stores fingerprint in daemon `state.refs` map and re-locates on action via tier-1..4 fallback | mutates DOM (`data-hcr-ref` attr) — visible to page scripts, can break CSP/MutationObservers, lost on re-render |
| Mode filters | `interactive | visible_only | form | dialog | navigation | errors | headings` | none — fixed list |
| Limit | `limit: u32` (default 40) + `truncated` flag | unbounded — emits every actionable role |
| Ordered tag/role inference | dispatches via `pi.inferRole`, `pi.isInteractiveEl` (a shared library) | inline `TAG_TO_ROLE` map + small `ACTIONABLE_ROLES` set |

### Ref stamping

gsd-browser **does not** stamp DOM. Refs are server-side: `state.refs: HashMap<String, Value>` keyed by `eN`, value = full fingerprint (tag, role, name, deepPath, contentHash, structuralSignature, frameIndex). On action, `resolve_snapshot_node` re-walks the DOM with a 4-tier locator:

```
tier 1: deepPath replay (shadow-aware structural path)
tier 2: selectorHints (any hint that resolves uniquely)
tier 3: role + accessible name match
tier 4: tag + contentHash OR tag + structuralSignature
```

Returns `{ ok: false, reason: "stale" }` if all four tiers fail.

We use a single `data-hcr-ref` CSS-selector lookup. If the page mutates between snapshot and act (SPA re-render, virtualised list, React reconcile), the stamped node's attribute is preserved by React's diff *only* for elements React's diff considers identical. For any mid-render re-mount, the attribute is lost and our `find_element` will return `Err`, but we report it as `BrowserError::Other("find ...")` not `StaleRef`.

### Action dispatch

gsd-browser dispatches via `act_on_snapshot_node` (`inspection.rs:1086-1260`) which runs **all action JS inside the page context** — synchronous within one `evaluate_expression`. Effects:

- `click`: `el.focus()` then `el.click()` (function call, not CDP `Input.dispatchMouseEvent`). Falls back to `dispatchEvent(MouseEvent('click'))` if `el.click` isn't a function.
- `fill`: handles three element classes:
  - `'value' in el` → set `value`, dispatch `input` + `change`. `slowly` mode walks char-by-char with `input` event per char.
  - `el.isContentEditable` → set `textContent`, dispatch `input`.
  - else → throw "element does not support text input".
- `submit: true` flag → dispatches `keydown`/`keyup` Enter + walks `el.form || closest('form')` and prefers `form.requestSubmit()` over `form.submit()`.
- `set_checked`: sets `el.checked = !!options.checked` directly + dispatches `change` + `input`.
- `select_option`: matches `option.label || option.value || normalizeText(textContent)`.

After every ref action, gsd-browser runs `settle_after_action` (MutationObserver-backed quiet-window detection) — see `settle.rs:23-56`. Only after settle does it return.

**Ours**: uses chromiumoxide's `Element::click()`, `type_str()`, `focus()`, `press_key()`, `hover()` — these go through CDP (`Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`). Different layer, different bug surface (see chromiumoxide gotchas below).

### Visibility detection

Theirs (`isVisible`):

```js
const style = getComputedStyle(el);
if (style.display === "none" || style.visibility === "hidden") return false;
const rect = el.getBoundingClientRect();
return rect.width > 0 && rect.height > 0;
```

Ours:

```js
function visible(el) {
  const r = el.getBoundingClientRect();
  if (r.width === 0 && r.height === 0) {
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
  }
  return true;
}
```

Our logic returns `true` for an element with zero rect IF computed style isn't `none`/`hidden`. That admits offscreen `position:absolute; left:-9999px` elements, `aria-hidden` overlays, `opacity:0` decorative wrappers, and elements clipped by `overflow:hidden` parents. Theirs is stricter. Neither handles `aria-hidden="true"` or `inert` or ancestor `display:none` checks. Both miss `opacity:0`.

### Filter list

Their actionable roles (`includeElement` mode `interactive`): `a, button, input, select, textarea, summary` (tags) — `pi.isInteractiveEl` from a shared library.

Ours (`ACTIONABLE_ROLES`):
```
button, textbox, link, checkbox, combobox, menuitem,
option, radio, searchbox, tab, switch, slider,
spinbutton, treeitem, heading, img
```

Discrepancies:
- We include **`heading`** and **`img`** as actionable. They're *informational*, not actionable. Including them inflates the snapshot 5–10x on content-heavy pages and pollutes the model's ref space with refs for `<h1>` and `<img>` that have no useful action.
- We miss **`<summary>`** (HTML disclosure widget — actionable, has implicit `role=button`).
- We miss **`<details>`** (toggleable).
- We miss **`<label>`** (clickable proxy for its associated input).
- We miss **`<a>` without href** (still has `role=link` via our explicit-role path but only if author set `role`; HTML semantic `<a>` without href still gets role from `TAG_TO_ROLE['A'] = 'link'` so this is fine).
- We miss **`menuitemcheckbox`**, **`menuitemradio`**, **`tabpanel`**, **`gridcell`**, **`columnheader`**, **`rowheader`**, **`treegrid`**.
- We miss **contenteditable** elements (no role, but actionable for `Fill`/`Type`).
- We miss **`<iframe>`** as a role (theirs walks into them; we don't even enumerate them).

### Error / stale-ref handling

gsd-browser:
- ref version mismatch → `"ref version mismatch: ref is vN but current snapshot is vM"` (specific).
- ref key not found in current store → `"ref @vN:eM not found in snapshot vN"`.
- DOM resolution failed (all 4 tiers) → `{ ok: false, reason: "stale" }` returned to caller.

Ours:
- `target.version != session.snapshot_version` → `BrowserError::StaleRef` (good).
- `last_snapshot_element_ids` doesn't contain id → `BrowserError::NotActionable` (semantically wrong — it's "ref id not in snapshot", not "exists but not actionable").
- DOM lookup fails (`find_element` returned Err) → `BrowserError::Other(format!("find {selector}: {e}"))`. **Bug**: there's no fallback locator. If the SPA re-rendered and cleared our `data-hcr-ref` attr in the new node, we hard-fail without trying role+name re-resolution. gsd-browser re-locates.

## playwright-mcp comparison

Tool surface — they expose 70+ tools. We expose 7. Of their snapshot/action set:

| playwright-mcp | ours | divergence |
|---|---|---|
| `browser_snapshot` | `browser_snapshot` | aligned |
| `browser_navigate` | `browser_navigate` | aligned |
| `browser_click` | folded into `browser_act { kind: "click" }` | divergent — agent must learn one-tool-many-kinds vs many-tools |
| `browser_type` | `browser_act { kind: "type" }` | "" |
| `browser_fill_form` | `browser_act { kind: "fill" }` (single field; no batch) | weaker — pwmcp can fill a list of fields atomically |
| `browser_press_key` | `browser_act { kind: "press" }` | "" |
| `browser_hover` | `browser_act { kind: "hover" }` | "" |
| `browser_select_option` | `browser_act { kind: "select" }` | "" |
| `browser_drag` + `browser_drop` | `browser_act { kind: "drag" }` (stub) | divergent + unimplemented |
| `browser_file_upload` | `browser_act { kind: "upload" }` (stub) | unimplemented |
| `browser_take_screenshot` | `browser_screenshot` | naming nit (lose `_take_`) |
| `browser_wait_for` | `browser_wait` | naming nit |
| `browser_close` | `browser_session { op: "close" }` | divergent |
| `browser_tabs` | not present | missing |
| `browser_console_messages` | provider trait stub only | missing tool |
| `browser_network_request(s)` | provider trait stub only | missing tool |
| `browser_evaluate` | provider trait stub (gated) | missing tool |
| `browser_handle_dialog` | not present | missing |
| `browser_resize` | not present | missing (viewport pinning is in `SessionSpec` but no live tool) |
| `browser_navigate_back/forward/reload` | provider trait stubs only | missing tools (only forward/back exist as `CapabilityMissing` defaults) |

ADR-028's "open question 2" (one-tool-many-kinds vs many-tools) was resolved unified for token economy. Defensible, but understand: an agent trained on playwright-mcp examples will instinctively call `browser_click(ref=...)` and our schema validator will fail it. Cost is documented in the harness skill prompt; check that's actually shipping.

The pwmcp ref shape is `e123` — a string, no `@v` prefix. They rely on Playwright's snapshot regeneration on every tool call (snapshot is implicit, not explicit). Ours has an explicit version (better for audit/replay) but more surface for the agent.

pwmcp's `browser_fill_form` accepts a list:
```
fields: [{ ref, value }, { ref, value }, ...]
```
A common pattern (login forms). Worth adopting.

## jcode comparison

`jcode` is a Firefox bridge via native messaging — not chromium. Its `BrowserTool::execute` dispatches to a long list of action strings (`status, setup, list_tabs, ..., snapshot, get_content, interactables, click, type, fill_form, ...`). Snapshot impl is in the bridge's JS, not in `browser.rs`. Their tool naming ("interactables" vs "snapshot") is split: snapshot = full DOM dump, interactables = filtered actionables. We collapse both into one. Defensible — gsd-browser also collapses.

Their `fill_form` takes `fields: Vec<BrowserField { selector, value, checked }>` — same multi-field pattern as pwmcp. Reinforces "we should accept arrays" as the consensus.

## chromiumoxide gotchas

Open bugs that materially affect us:

- **#320 — `Element::click()` hangs indefinitely** (confirmed reproducible on `https://example.com`). Workaround: call JS directly: `page.evaluate("document.querySelector('...').click()")`. We use `element.click().await` in `ActionKind::Click` — **we will hang on certain elements**. gsd-browser dodges this entirely by using `evaluate_expression` + JS `el.click()`.

- **#292 — `find_element` waits indefinitely for invisible elements**. Our `act` calls `page.find_element(selector)` with no timeout wrapper. If the snapshot stamped an element but a CSS animation hid it before act runs, we hang. gsd-browser doesn't use `find_element` at all — it does `queryAllDeep` inside an `evaluate_expression` which has a 30s timeout (`INSPECTION_TIMEOUT`).

- **#296 — no API to retrieve element by NodeId** — design impact: stamping is the only stable bridge between snapshot and action via chromiumoxide's high-level `Element` API. Our `data-hcr-ref` attribute approach is the *correct* shape for chromiumoxide specifically. gsd-browser sidesteps this by staying in JS-eval-land.

- `Element::call_js_fn(fn_decl, await_promise: bool)`. Our `Fill` clear and `SetChecked` and `Select` pass `false`. For `SetChecked` doing `this.click()` synchronously this is fine. For any future `Select` that triggers async option-change handlers, `false` will return before the handler resolves. Worth flagging for v0.2 form-fill cases that wait on remote validation.

- `Element::type_str` types via CDP `Input.insertText` + per-char `Input.dispatchKeyEvent`. This **doesn't fire React's synthetic event for `value`** in some controlled inputs — React's `_valueTracker.getValue()` is bypassed. Symptom: agent types into a React form, the visible value updates, but the form's controlled state doesn't and submission sends empty. gsd-browser sidesteps this by using JS `el.value = text; dispatchEvent(new Event('input', {bubbles:true}))` directly, which trips React's `valueTracker`. **High-likelihood real-world bug for any React/Vue controlled-input flow**.

- `BrowserConfig::with_head()` — we set headless via `!headless` toggling `with_head()`. Confirmed correct API.

## Bugs / gaps in our impl (HIGH-priority)

1. **`Element::click()` hang on real pages** (chromiumoxide #320). Reproducer: any anchor on `https://example.com`. Fix: replace `element.click().await` with `element.call_js_fn("function() { this.click(); }", false)` or `page.evaluate(format!("document.querySelector('[data-hcr-ref=\"{}\"]').click()", ref))`.

2. **`type_str` doesn't trip React's value tracker**. Reproducer: any React-controlled `<input>` (e.g., `https://react.dev` search box). Fix: implement `Type`/`Fill` via JS just like gsd-browser:
   ```js
   el.value = text;
   el.dispatchEvent(new Event('input', { bubbles: true }));
   el.dispatchEvent(new Event('change', { bubbles: true }));
   ```

3. **No shadow DOM walk**. Reproducer: any page using web components (`youtube.com`, modern Salesforce, GitHub's new file viewer). `querySelectorAll('*')` does not descend into `shadowRoot`. Snapshot will show ~zero actionable elements on these pages. Fix: implement `collectQueryRoots` that recurses `el.shadowRoot` (gsd-browser `inspection.rs:206-215`).

4. **No iframe walk**. Reproducer: any page embedding Stripe Elements, Google reCAPTCHA, YouTube embed, OAuth popups. We only see the parent frame. Fix: enumerate `window.frames` like gsd-browser's `collectFrameEntries`. Cross-origin frames must be reported as boundaries (cannot enter), not silently skipped.

5. **`heading` and `img` are not actionable** but we emit them. On a Wikipedia article page, this means hundreds of `@v1:eN` refs the agent has to scroll through. Fix: drop `heading` and `img` from `ACTIONABLE_ROLES` unless they have a click handler / `tabindex` / `role=button`.

6. **No snapshot limit / truncation**. A 10k-node page produces a 10k-element snapshot — token-blasting. Fix: parameter `limit` (default 40 like gsd-browser) + `truncated: bool` flag.

7. **`find_element` hang on invisible/animating elements** (chromiumoxide #292). We never wrap it in `tokio::time::timeout`. Fix: wrap with a 5s timeout and return `BrowserError::StaleRef` on timeout.

8. **`NotActionable` is misnamed**. When `last_snapshot_element_ids` doesn't contain the requested id, we return `BrowserError::NotActionable`. The semantic is "ref not in current snapshot" — should be `StaleRef`. As-is, error mapping confuses the agent's retry logic.

## Bugs / gaps in our impl (MEDIUM)

9. **No DOM-settle wait after action**. gsd-browser installs a `MutationObserver` and polls `__piMutationCounter` post-action until quiet (`settle.rs`). We just snapshot immediately. On any SPA route change, our post-act snapshot captures a half-rendered DOM — the agent acts on stale state next turn.

10. **`Fill` doesn't handle contentEditable**. We only call `this.value = ''` if `'value' in this`. ContentEditable divs (Slack message input, Notion blocks, ProseMirror editors) are silently no-op. gsd-browser handles `el.isContentEditable` branch.

11. **Stamping mutates page DOM**. Some sites use strict CSP attribute hashes or have `MutationObserver` listening to attribute changes — we trip those observers every snapshot, potentially causing infinite re-render loops in well-instrumented apps. Server-side ref tracking (gsd-browser) avoids this.

12. **No frame ownership in error messages**. When a click fails in an iframe (we don't even support that yet, but when we add it), we won't know which frame. gsd-browser carries `frameLabel` + `frameUrl` through every result.

13. **`Select` matches by `option.label || option.text || option.value`**. gsd-browser also includes `normalizeText(option.textContent || "")`. For `<option>foo&nbsp;bar</option>`, `text` and `textContent` differ — ours can miss this.

14. **`AssertPredicate::TextVisible` only searches `name`**. It iterates `e.name` — misses any text content not surfaced as accessible name (long paragraph in a `<p>`, error message in a non-`role=alert` div). Should use `document.body.innerText` like gsd-browser's `text_query`.

15. **`WaitCondition::SelectorVisible/Hidden/UrlContains/UrlMatches/TextVisible/TextHidden/RefVisible/NetworkIdle` all fall through to "sleep timeout_ms"**. From `chrome_provider.rs:576-585`. The trait promises these, the impl is a stub. This will silently no-op — agent waits 30s and continues regardless of condition.

16. **`act` calls `self.snapshot()` while holding/releasing locks — race**. `act` releases the session lock to clone `page`, dispatches CDP, then calls `snapshot` which re-acquires the lock. Between these, another concurrent caller could call `snapshot` and bump the version. The version returned to our act caller would be `act_version + 2`, surprising. Probably benign in single-tenant usage; can bite under multi-agent.

17. **Snapshot doesn't expose `enabled` state**. gsd-browser tracks `el.disabled`. An agent clicking a disabled button gets a silent no-op. Add `enabled` to `SnapshotElement` and surface it.

## Bugs / gaps in our impl (LOW / nitpicks)

18. **`accessibleName` has no `aria-labelledby` resolution**. Theirs delegates to `pi.accessibleName` which may; ours only checks `aria-label`, then placeholder, then `<label for>`, then text content.

19. **Script string interpolation for `version`** uses Rust `format!` — fine, but we don't escape. `SnapshotVersion` is a `u64`, so safe by type. Still, makes the boundary fragile to refactor.

20. **`raw_node_count` in snapshot meta is `document.querySelectorAll('*').length` from the JS** — for a shadow-heavy page this undercounts. Cosmetic, but the metric label is misleading.

21. **`Tool::name()` returns `&str` to a static literal** — fine.

22. **`browser_screenshot` returns raw PNG bytes wrapped in `Vec<u8>`** but the harness tool layer must base64-encode for ACP/JSON. Verify `tools.rs` does this; if it returns binary as `String`, JSON serialization explodes silently.

23. **`SessionSpec.viewport`** is plumbed but `chrome_provider.rs` never reads it. The session always uses Chrome default viewport. ADR-028 §"Reproducibility" pins viewport — currently broken.

24. **No `browser_navigate_back / forward / reload` tools** even though provider traits exist. Easy win.

## Things we got right

- **Versioned ref wire format** `@v<N>:e<id>`. Identical to gsd-browser. Audit-friendly, parser symmetric, parse-rejects-garbage tested.
- **Custom `Serialize/Deserialize` rendering refs as strings** rather than nested objects. Saves tokens, makes JSON readable.
- **`BrowserContext` carries `tenant_id`** — provider verifies on every call. Cleaner than gsd-browser which is single-tenant.
- **Capability table** with `CapabilityStability` levels. gsd-browser doesn't have this; pwmcp doesn't either. Useful for substrate evolution.
- **Provider trait is I/O-free, in `*-core`** — clean Layer-2 boundary per ADR-016.
- **`act` returns `(ActionOutcome, Snapshot)` inline** — saves a round-trip vs pwmcp where snapshot is a separate tool call.
- **`Snapshot::lookup` rejects stale-version refs at the type level** — defense in depth.
- **`AssertPredicate` enum is a clean DSL** — pwmcp's `browser_verify_*` family is more sprawling.
- **Tool naming aligns with playwright-mcp prefix** (`browser_*`) — discoverable for agents trained on pwmcp examples.

## Things they got wrong (and we should not copy)

- **gsd-browser stamps mutation counter as `__piMutationCounter` global** — name-collides with any page using `pi` namespace (e.g., D3 plots, scientific viz tools). Use a UUID-suffixed or symbol-keyed name.
- **gsd-browser's tier-1 `deepPath` resolver is brittle** — it stores `Array.from(parent.children).indexOf(current)`. For any list reordering (drag-sort, virtualised list scroll), tier-1 silently resolves to the wrong element. They mitigate with tier-2..4 fallback; we'd be lifting the whole 4-tier machinery to gain it. Cheaper to use stable refs (our approach) when possible and fall back to fingerprint only on stamp loss.
- **playwright-mcp ships 70+ tools**. Tool-list bloat is real; their own README admits it (the CLI+SKILLS pitch). Our 7-tool surface is closer to the right size for an LLM context budget.
- **gsd-browser's snapshot defaults `limit=40`** — too low for some real workflows (long forms with 50 fields). Make it configurable; don't hardcode 40.
- **gsd-browser routes everything through `evaluate_expression`** — adds 30ms+ overhead per action vs CDP `Input.dispatchMouseEvent`. For high-frequency scripted flows, CDP is faster. Hybrid (CDP click for primary path, JS fallback on hang) is the better answer.

## Recommended fixes ordered by impact

1. **Replace `element.click().await` with JS-eval click** (chromiumoxide #320). One-line fix, eliminates the most common hang. Gates everything else.
2. **Fix `Fill`/`Type` to dispatch `input`/`change` events via JS** so React/Vue controlled inputs commit. Without this, every modern web app silently fails post-fill.
3. **Drop `heading` and `img` from actionable role list**. Add `summary`, `details`, `label`. Add contenteditable fallback in `Fill`.
4. **Add shadow DOM walk** (`collectQueryRoots` recursion). Without this, web-component sites are blank.
5. **Add iframe enumeration with cross-origin boundary reporting**. Without this, OAuth/payment/recaptcha flows are blank.
6. **Wrap `find_element` with `tokio::time::timeout(5s)` returning `StaleRef`** (chromiumoxide #292).
7. **Implement DOM-settle wait** (MutationObserver post-action) before returning the post-act snapshot. SPAs need this.
8. **Fix `WaitCondition` stub**: implement `SelectorVisible/Hidden`, `UrlContains/Matches`, `TextVisible/Hidden`, `RefVisible`. The trait lies right now.
9. **Rename `NotActionable` → `StaleRef` for the "id not in current snapshot" branch**. Two-line fix; clarifies retry semantics.
10. **Add `limit` parameter + `truncated` flag to snapshot**. Token economy on large pages.
11. **Add `enabled` to `SnapshotElement`** and skip disabled elements (or surface state).
12. **Use absolute (frame-corrected) bounds in snapshot**. Required prerequisite for iframe support; also unblocks visual-debug overlays.
13. **Multi-field `Fill` variant**: `ActionKind::FillFields { fields: Vec<(ElementRef, String)> }` — matches pwmcp + jcode + most real form flows.
14. **Honour `SessionSpec.viewport`** in `ChromeConfig` / page init. Currently silently ignored; ADR-028 reproducibility broken.
15. **Add `back/forward/reload/tabs/console/network` tools** at the harness layer. Provider trait is ready; tool layer just needs the wrappers.
16. **Stop mutating DOM on snapshot**. Migrate to gsd-browser-style server-side ref store with a 4-tier locator. This is large (~1 week) but is the only way to be correct on apps that observe attribute mutations. Defer to v0.2 if migration is expensive; meanwhile, namespace the attribute (`data-hivecore-ref` not `data-hcr-ref` — the `hcr` abbreviation is opaque) and document the trust boundary.

Bottom line: the architecture is sound, the surface is well-scoped, the multi-tenant + capability + versioned-ref discipline is ahead of the prior art. The JS walker is the load-bearing piece that needs the most work — half a dozen real-page failure modes today (React inputs, web components, iframes, shadow DOM, image/heading bloat). Fixes 1–6 are mechanical and should land before any user-facing demo.
