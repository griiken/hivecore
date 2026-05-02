# Post-fix validation — implementation vs OSS prior art (round 2)

## Verdict

**Mostly correct, with one minor data-shape gap and several documented v0.2 deferrals.** The six HIGH-priority fixes (#1, #2, #3, #4, #5, #6) and #8 are applied semantically correctly and in some cases (React `_valueTracker` setter dance, opacity check) are *stricter* than the gsd-browser reference. Two real issues remain: (a) `SnapshotElement.enabled` is collected in JS but **not propagated into the Rust struct** — the field is dropped on the floor; (b) the 50ms settle is a substantial regression vs gsd-browser's MutationObserver-based adaptive loop and will misfire on real SPA route changes. Neither blocks v0.1 demo, but (a) should be a same-day fix and (b) should be tracked as the first v0.2 task. NotActionable variant is now dead code — either remove or wire up.

## Fix-by-fix verification

### Fix #1 — JS-eval click (chromiumoxide #320)

**Status: CORRECT** (improvement over reference for read-only safety; minor robustness gap)

**Our code** (`chrome_provider.rs:559-563`):
```rust
js_call(&element,
    "function() { try { this.focus({preventScroll:true}); } catch(e) {} this.click(); }",
).await?;
```
Plus `element.scroll_into_view().await` *before* the JS call.

**gsd-browser** (`inspection.rs:567-583`):
```js
if (el.scrollIntoView) {
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
}
case "click":
    if (typeof el.focus === "function") el.focus();
    if (typeof el.click === "function") {
        el.click();
    } else {
        el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, view: context.win }));
    }
```

**Notes.**
- Both call `.click()` (DOM method) which fires React `onClick` correctly — confirmed by reference impl using the same.
- We use `focus({preventScroll:true})` — defensible improvement; gsd-browser does plain `focus()` which can re-scroll the page after our explicit `scroll_into_view`.
- **Robustness gap:** gsd-browser falls back to `dispatchEvent(MouseEvent('click'))` when `el.click` isn't a function. We don't. This matters for non-HTMLElement clickable targets (SVG, MathML, custom elements that override the prototype). On modern HTML buttons it never matters. **Suggest copying the fallback** — three lines and it preserves test parity.
- We grep'd `chrome_provider.rs` for `Element::click()` — only the doc comment mentions it; no live calls. Replacement is total.

### Fix #2 — JS-eval fill (React `_valueTracker`)

**Status: CORRECT — stricter than gsd-browser**

**Our code** (`chrome_provider.rs:599-616`):
```js
const setter = Object.getOwnPropertyDescriptor(proto, 'value') &&
               Object.getOwnPropertyDescriptor(proto, 'value').set;
if (setter) setter.call(this, v); else this.value = v;
this.dispatchEvent(new Event('input', {bubbles:true}));
this.dispatchEvent(new Event('change', {bubbles:true}));
```

**gsd-browser** (`inspection.rs:599-616`):
```js
if ("value" in el) {
    if (clearFirst) { el.value = ""; el.dispatchEvent(new Event("input", {bubbles:true})); }
    el.value = text;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
}
```

**Notes.**
- gsd-browser does **plain `el.value = text`** — does NOT use the prototype setter trick. **Our impl is more correct for React 16+** because direct assignment desyncs `_valueTracker`, causing React to skip the synthetic event. The setter-via-prototype dance is the canonical fix (documented widely in React testing guides).
- ContentEditable branch: both impls set `textContent` and dispatch `input`. Match.
- Fill semantics match (replace). Type semantics match (append). Slow-mode (char-by-char with per-char input event, used for autocomplete/typeahead inputs) — gsd-browser has it, we don't. **Note as v0.2 gap.**
- Submit-on-fill (gsd-browser dispatches Enter keydown/keyup + form.requestSubmit when `submit:true`) — we don't have this option at all. Acceptable for v0.1.

### Fix #3 — Role filter

**Status: OVER-CORRECTED in two places, CORRECT in spirit**

**Our roles** (chrome_provider.rs:263-268): button, textbox, link, checkbox, combobox, menuitem, menuitemcheckbox, menuitemradio, option, radio, searchbox, tab, switch, slider, spinbutton, treeitem, gridcell, columnheader, rowheader, tabpanel.

**gsd-browser interactive mode** (`inspection.rs:889`):
```js
const interactive = pi.isInteractiveEl ? pi.isInteractiveEl(el)
                  : ["a","button","input","select","textarea","summary"].includes(tag);
```
Tag-based fallback only. The `pi.isInteractiveEl` lib lookup falls back to a slightly broader role test but the canonical reference is the tag set above.

**Concerns.**
- **`tabpanel` is NOT actionable** — it's a region container. Including it produces noisy refs the agent can click but won't get a sensible result. **Recommend removing.**
- **`gridcell` / `columnheader` / `rowheader` are debatable.** Sortable column headers are clickable, but plain data cells are not. gsd-browser doesn't include them in interactive mode. We will get false positives on data tables.
- `summary`/`details`/`label`/contenteditable additions — all genuinely actionable. Match gsd-browser's fallback list (`summary`).
- `menuitemcheckbox`/`menuitemradio` — correct addition; both fire on click.
- **Mode gap:** gsd-browser supports 7 modes (`interactive | visible_only | form | dialog | navigation | errors | headings`); we have one implicit "interactive-ish" mode. Document as v0.2 gap. SnapshotMode parameter type already exists in the public API (per AGENTS.md hint) — wiring is the work.

### Fix #4 — Shadow DOM walk

**Status: CORRECT — direct port of gsd-browser**

**Our code** (chrome_provider.rs:331-339):
```js
function collectQueryRoots(scope, out) {
    out.push(scope);
    const all = scope.querySelectorAll ? scope.querySelectorAll('*') : [];
    for (const el of all) {
        if (el.shadowRoot) collectQueryRoots(el.shadowRoot, out);
    }
}
```

**gsd-browser** (`inspection.rs:206-215`): identical structure, returns array instead of push-via-out-param. Functionally equivalent.

**Notes.**
- Closed shadow roots (`{mode: 'closed'}`) — neither impl can pierce. Document gap (it's a browser-platform limitation, not an impl bug).
- Performance: O(N) querySelectorAll per shadow root. Fine.
- One subtle thing: we `push(scope)` *first* then walk children. gsd-browser does the same. Match.

### Fix #5 — Iframe walk

**Status: CORRECT for v0.1; INCOMPLETE for production**

**Our code** (chrome_provider.rs:342-356): walks `iframe.contentDocument` for same-origin frames; lumps cross-origin into `cross_origin_frames`.

**gsd-browser** (`inspection.rs:71-202`): walks the entire `window.frames` tree recursively up to depth 10, attaches `frameLabel`, `frameIndex`, `frameName`, `frameUrl` to **every element record** so the daemon can route the action back to the originating frame's document.

**Concrete failure mode for our flat snapshot.**
An agent receives `{ref: "@v3:e7", role: "button", name: "Send"}` for an element that lives in an iframe. The agent calls `act("@v3:e7", click)`. We call `page.find_element("[data-hivecore-ref=\"@v3:e7\"]")`. **chromiumoxide's `Page::find_element` only queries the top document** — same-origin iframe elements ARE stamped with the data attribute by our JS walker, but the CDP `DOM.querySelector` from `Page` won't traverse iframe content documents by default. **This means: same-origin iframe elements show up in snapshot but `act()` will fail with StaleRef** (find_element returns nothing → Ok(Err) branch → StaleRef). HIGH-impact gap on any real page with same-origin iframes (auth callback frames, payment iframes from same TLD, etc.).

**Status:** functionally **broken for iframes**. Either (a) document iframes-not-yet-supported and skip them in walk, or (b) carry frame info and dispatch via per-frame Page handle. Recommend (a) for v0.1: in `collectFrames`, push iframes into `cross_origin_frames` regardless and only walk main `document`. Better than the false-positive shape we have now.

### Fix #6 — find_element timeout

**Status: CORRECT**

**Our code** (chrome_provider.rs:523-537): `tokio::time::timeout(Duration::from_secs(5), page.find_element(selector))`.

**gsd-browser** uses `INSPECTION_TIMEOUT = 30s` for the whole eval (settle.rs constants section). Wrapping just `find_element` at 5s is reasonable — by the time we reach `find_element`, the element should already exist in DOM (we just stamped it 50ms ago). Five seconds is generous.

**On timeout we return `StaleRef`.** Semantically defensible: from caller's POV, the ref no longer resolves. A purist could argue for a separate `Timeout` variant for observability — minor.

### Fix #8 — NotActionable → StaleRef

**Status: CORRECT but variant is now DEAD CODE**

```
$ grep -rn "NotActionable" crates/
crates/hivecore-browser-runtime/src/chrome_provider.rs:508:            // (was NotActionable). [comment only]
crates/hivecore-browser-core/src/error.rs:19:    NotActionable(String),
```

**No live caller anywhere.** Either:
- Remove the variant (clippy will tell users on next compile, breaking change for any external consumer of `BrowserError` matching exhaustively), OR
- Wire it up for the legitimate disabled-element case: in `act()`, after `find_element` succeeds, run a JS `isEnabled` check; if false → `NotActionable`.

Recommend the latter — we already collect `enabled` per element in JS (chrome_provider.rs:385) but throw it away (see Snapshot shape gap below). Hooking it through gives `NotActionable` a real semantic.

## Settle delay gap

**Concrete failure mode for our 50ms placeholder vs gsd-browser's MutationObserver loop:**

gsd-browser installs a `MutationObserver` once per page (settle.rs:25-42) that bumps `window.__piMutationCounter` on every DOM change. After an action, it polls in adaptive windows: 30ms quiet window if mutations have been zero for >60ms, otherwise full quiet window (default 250ms).

**On a slow SPA route change (e.g. React Router v6 lazy route):**
- User clicks "Submit" → React schedules a state update.
- Microtask runs: form transitions to "loading" UI (DOM mutation #1, ~10ms).
- Promise resolves (~80ms): network reply arrives.
- React commits new tree (DOM mutation #2, ~120ms).
- Lazy chunk loads new route (~200ms): more mutations.

**Our 50ms sleep:** we re-snapshot at +50ms — between mutation #1 and #2. The snapshot the agent sees is the "loading" state, NOT the post-submit state. The agent will see a spinner or stale form, conclude the submit "didn't do anything", and act incorrectly.

**Severity: HIGH on real SPAs, LOW on data: URLs and static pages.** The smoke and act examples both use data: URLs so they don't catch this. v0.2 priority.

Minimal port: install the observer in `navigate()` once, poll `window.__piMutationCounter` in a 5–500ms adaptive loop in `act()` before re-snapshot. ~80 LOC.

## Visibility behaviour change

**Our `isVisible`** (chrome_provider.rs:313-323): rejects `display:none`, `visibility:hidden|collapse`, `opacity:0`, `[hidden]` attribute, zero bounding rect.

**gsd-browser's** (`inspection.rs:26-32`): rejects `display:none`, `visibility:hidden`, zero bounding rect. Does NOT check `opacity` or `[hidden]`.

**Did we lose elements?**
- `opacity:0` — common pattern for transition-fade elements, "screen reader only" content using `opacity:0;position:absolute` (rare; usually uses `clip-path` or `width:1px`), and **disabled buttons in some design systems**. Net: we lose a small number of off-screen-but-DOM-present elements gsd-browser would surface. For an **agent that should only act on what a sighted user sees**, this is *correct* and *better than reference*.
- `[hidden]` HTML attribute — equivalent to `display:none` per spec. Our explicit check is redundant but not wrong.

**Conclusion:** stricter, intentional, correct. No regression.

## Frame-aware refs gap

Already covered under Fix #5. Concrete: same-origin iframe elements appear in snapshot, but `Page::find_element` can't reach them, so `act()` fails StaleRef despite the snapshot being fresh. **HIGH-priority residual.**

## Residual HIGH-priority bugs

1. **Same-origin iframe elements appear in snapshot but cannot be acted on** — `find_element` doesn't traverse content documents. Either drop iframe walking until per-frame routing is built (recommended v0.1 patch), or carry frame metadata and route via per-frame Page handle (v0.2).
2. **`SnapshotElement.enabled` field collected in JS, dropped in Rust.** chrome_provider.rs:385 emits `enabled: isEnabled(el)`; chrome_provider.rs:467-474 builds `SnapshotElement` without it; snapshot.rs has no `enabled` field. The validation report explicitly asked us to verify this — it's missing. Minor structural fix: add `pub enabled: bool` to `SnapshotElement`, default true, propagate from JS.
3. **50ms settle is wrong on SPAs.** v0.2 must replace with MutationObserver loop.

## Residual MED/LOW

- **MED.** `tabpanel` / `gridcell` / `columnheader` / `rowheader` over-included in actionable roles — false-positive refs on regions and data cells.
- **MED.** `Element::click()` fallback for non-HTMLElement targets missing (gsd-browser dispatches synthetic MouseEvent).
- **MED.** `NotActionable` variant is dead code — either remove or wire up via `enabled` check.
- **LOW.** No slow-mode (per-char) typing — autocomplete/typeahead inputs may not fire all listeners.
- **LOW.** No submit-on-fill option (Enter dispatch + form.requestSubmit).
- **LOW.** `wait_for` only implements Delay/Load — Selector/Role/Text/Network/Idle all degrade to Delay(timeout_ms). Documented in source.
- **LOW.** Drag and Upload are stubs.
- **LOW.** Closed shadow roots — platform limitation, document only.
- **LOW.** No mode parameter (interactive/form/dialog/navigation/errors/headings) — single implicit interactive mode only.

## Recommended sign-off action

**Apply these 3 fixes first**, then mark validation passed:

1. **Add `pub enabled: bool` to `SnapshotElement`**, default true, propagate from JS walker. ~5 LOC.
2. **Drop iframe walking** for v0.1: in `collectFrames`, treat all iframes as `cross_origin_frames` (don't push to `frames`). Avoids the silent-fail StaleRef on same-origin iframes. ~3 LOC delete. Track per-frame routing as v0.2 work item.
3. **Remove `tabpanel`, `gridcell`, `columnheader`, `rowheader`** from ACTIONABLE_ROLES. ~4 LOC delete.

After those, AGENTS.md / CHANGELOG can mark "browser harness post-fix validation passed; v0.2 backlog: settle observer, frame-aware refs, slow-type, submit-on-fill, snapshot modes."
