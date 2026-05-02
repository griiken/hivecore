# Browser tool prior-art

Survey of how working OSS coding-agent harnesses expose browser control to LLMs. Date: 2026-05-01.

## gsd-browser

- **URL.** https://github.com/gsd-build/gsd-browser
- **License.** Apache-2.0 (also dual MIT — `LICENSE-APACHE` + `LICENSE-MIT` in the tree). Compatible with hivecore (MIT-OR-Apache-2.0).
- **Language.** Rust. Workspace: `members = ["common", "cli"]`. Binary `gsd-browser`.
- **Stars.** 211.
- **Architecture.**
  - Single static binary CLI. Talks **Chrome DevTools Protocol** via `chromiumoxide = "0.9"`.
  - Background **daemon** model: first command auto-spawns a daemon process; subsequent CLI invocations attach over a local socket. State (current page, snapshot version, refs) persists across CLI calls.
  - `daemon health` reports state without auto-starting. `daemon start` is opt-in pre-warm.
  - Skill ships in-tree at `gsd-browser-skill/SKILL.md` — a Claude-Code-style skill (frontmatter `name`, `description`, `allowed-tools: Bash(gsd-browser:*)`). Agent reads SKILL.md; tool surface is just the Bash CLI.
- **Action surface (63 commands).** From `README.md` "Command Surface":
  - Navigation — `navigate`, `back`, `forward`, `reload`
  - Logs/JS — `console`, `network`, `dialog`, `eval`
  - Interaction — `click`, `type`, `press`, `hover`, `scroll`, `select-option`, `set-checked`, `drag`, `set-viewport`, `upload-file`
  - Inspection — `accessibility-tree`, `find`, `page-source`
  - Waits — `wait-for` (11 conditions: `selector_visible`, `selector_hidden`, `url_contains`, `network_idle`, `delay`, `text_visible`, `text_hidden`, `request_completed`, `console_message`, `element_count`, `region_stable`)
  - Snapshots & refs — `snapshot`, `get-ref`, `click-ref`, `hover-ref`, `fill-ref`. Refs are **versioned** (`@v1:e1`); after navigation, old refs are stale and must be re-snapshotted.
  - Assertions/batching — `assert`, `diff`, `batch`
  - Pages/frames — `list-pages`, `switch-page`, `close-page`, `list-frames`, `select-frame`
  - Forms/semantic — `analyze-form`, `fill-form`, `find-best`, `act` (15 built-in semantic intents)
  - Diagnostics — `timeline`, `session-summary`, `debug-bundle`
  - Output — `screenshot`, `zoom-region`, `save-pdf`
  - Visual regression — `visual-diff`
  - Extraction — `extract` (structured)
  - Network control — `mock-route`, `block-urls`, `clear-routes`
  - Device/state — `emulate-device`, `save-state`, `restore-state`
  - Auth vault — `vault-save`, `vault-login`, `vault-list` (encrypted with `aes-gcm` + `argon2`)
  - Recording/traces — `generate-test`, `har-export`, `trace-start`, `trace-stop`
  - Safety/admin — `action-cache`, `check-injection`, `daemon`
- **Concrete agent invocation.** From `SKILL.md` core workflow:
  ```bash
  gsd-browser navigate https://example.com/form
  gsd-browser snapshot                         # → @v1:e1 [input email], @v1:e2 [input password], @v1:e3 [button "Submit"]
  gsd-browser fill-ref @v1:e1 "user@example.com"
  gsd-browser fill-ref @v1:e2 "password123"
  gsd-browser click-ref @v1:e3
  gsd-browser wait-for --condition network_idle
  gsd-browser snapshot                         # MUST re-snapshot — old refs stale
  ```
  All output supports `--json` for programmatic parsing.
- **Auth model.** Native `vault-save` / `vault-login` for credential reuse (AES-GCM + Argon2 KDF). `save-state` / `restore-state` for cookie/localStorage capture. Browser is launched/managed by the daemon; not designed to attach to the user's existing Chrome window by default.
- **Multi-agent / parallel handling.** Single-daemon, single-session model; multi-page within a session via `list-pages` / `switch-page`. Parallel agents would each need their own daemon (separate socket / data dir) — not built-in. Tester-army fan-out is not a first-class concept.
- **Trust/safety.** Explicit `check-injection` command — scans page content for prompt-injection markers before the agent reads it back into context. `action-cache` deduplicates expensive ops. `--json` keeps page text out of agent context unless explicitly fetched.
- **Structured output.** Accessibility tree is the primary surface (`snapshot`, `accessibility-tree`). Screenshots are secondary (used for visual regression, not interaction). Refs over selectors.
- **Skill exposure.** Yes — `gsd-browser-skill/SKILL.md` with Claude-Code frontmatter. The skill is the agent-facing API; the CLI is the runtime.
- **License-vendor compatibility for hivecore.**
  - **Steal patterns: yes** — daemon + ref-versioned snapshot + skill is the right shape for our `Tool` trait + skill system.
  - **Steal code: yes** — Apache-2.0 lets us vendor verbatim with attribution (precedent ADR-024 already vendors Codex/Zed prompts under Apache-2.0).
  - **Best fit: layer-3 reference impl.** Native `chromiumoxide` daemon + thin `Tool` wrapper.

## jcode browser bridge

- **URL.** https://github.com/1jehuang/jcode
- **License.** MIT. Compatible with hivecore.
- **Language.** Rust. Workspace with 30+ `jcode-*` crates.
- **Architecture.**
  - **Browser Provider Protocol** (BPP) — formal protocol spec at `docs/BROWSER_PROVIDER_PROTOCOL.md`. Defines message semantics, transport-agnostic ("direct Rust trait calls", stdio JSON-RPC, local socket, or wrapped remote API). Recommended envelope: JSON-RPC-shaped.
  - **Provider abstraction.** Multiple browser providers can satisfy the same protocol. Today: `firefox_agent_bridge` (the "Firefox Agent Bridge" — a Firefox extension + native messaging host installed under `~/.jcode/browser/`). Future: chrome / safari / edge slots reserved.
  - **Native messaging.** Firefox extension (`.xpi`) ↔ native host binary (stdio JSON, `connect_native` per Mozilla NM spec) ↔ jcode CLI. Per-session UNIX socket; PID file at `~/.jcode/browser/runtime/<session>.pid`.
  - **Source paths.** `src/browser.rs` (provider lifecycle, install, native-host registration), `src/tool/browser.rs` (Tool impl + provider registry).
- **Tool surface (one `browser` tool, ~21 actions).** Single tool with `action` string enum (from `BrowserTool::parameters_schema`):
  ```
  status, setup, list_tabs, new_tab, select_tab, get_active_tab,
  list_frames, open, snapshot, get_content, interactables,
  click, type, fill_form, select, wait, screenshot, eval, scroll,
  upload, press, provider_command
  ```
  Plus pass-through `provider_command` for provider-native methods (e.g. `firefox.install_extension`).
- **Normalized core methods (BPP required).** `provider.describe`, `provider.status`, `session.ensure`, `session.close`, `page.open`, `page.snapshot`, `page.click`, `page.type`, `page.wait`, `page.screenshot`. Optional groups: navigation (`page.goto`, `page.back`, `page.forward`, `page.reload`), keyboard/forms, tabs/pages, introspection (`page.eval`, `network.list`, `console.list`, `storage.get`, `cookie.list`), files/downloads.
- **Concrete agent invocation.** Tool call with JSON args:
  ```json
  {"tool": "browser",
   "action": "open",
   "browser": "firefox",
   "url": "https://example.com",
   "wait_until": "networkidle"}
  ```
  Snapshot returns flattened `elements: [{element_ref:"el_2", role:"link", name:"...", actionable:true, selector_hint:"a"}]` — opaque provider-issued refs (same idea as gsd-browser's `@v1:e1`).
- **Auth model.** Reuses the user's actual Firefox profile (the extension lives in their browser). Capability `persistent_profile` advertises this. Capability `attach_existing_browser` for connecting to a running browser.
- **Multi-agent / parallel handling.** Per-session sockets keyed by sanitized session_id (`session_socket_path(name)`). Each jcode session gets its own bridge instance. Provider's `session.ensure` carves an "isolated session or attachment scope" — supports parallel sessions per BPP.
- **Trust/safety.** Capability schema declares stability tiers (`stable | experimental | not_implemented`) per method. Diagnostic-item model surfaces missing extensions/permissions. Provider extensions namespaced (`firefox.*`, `cdp.*`) so passthrough is explicit.
- **Structured output.** Accessibility/DOM snapshot with `actionable` filtering. Snapshot formats: `annotated`, `text`, `textFast`, `html`, `title`.
- **Integration shape.** Single Rust `Tool` trait impl in jcode core. Provider trait (`id`, `supported_browsers`, `status`, `setup`, `ensure_ready`, `execute`) lets new browsers register.
- **License-vendor compatibility for hivecore.**
  - **Steal patterns: yes — BPP is the most directly applicable artifact.** A formal provider-abstraction protocol with versioning rules, capability schema, and certification tiers is exactly what a multi-tenant substrate wants. We can lift it nearly verbatim.
  - **Steal code: yes** (MIT). The Firefox-extension half is jcode-specific; the protocol + provider trait + native-host bootstrap are reusable.
  - **Best fit: protocol blueprint for layer-2 `BrowserProvider` trait, with gsd-browser as the first conforming provider.**

## Microsoft playwright-mcp

- **URL.** https://github.com/microsoft/playwright-mcp
- **License.** Apache-2.0 (Microsoft default). Compatible.
- **Language.** TypeScript / Node.js 18+. Distributed as `@playwright/mcp`.
- **Architecture.**
  - **MCP server.** stdio JSON-RPC speaking the Model Context Protocol. Any MCP client (Claude Desktop, Cursor, Cline, VS Code, Zed, etc.) mounts it via `npx @playwright/mcp@latest`.
  - Wraps Playwright (Chromium/Firefox/WebKit driver). Browser runs as Playwright subprocess.
  - **Accessibility-tree-first.** Explicitly: "Uses Playwright's accessibility tree, not pixel-based input." `browser_snapshot` is the canonical "see the page" call; `browser_take_screenshot` is for visual confirmation only ("You can't perform actions based on the screenshot").
- **Tool surface (~35 tools, all `browser_*`).** From auto-generated README list:
  - Interaction: `browser_click`, `browser_type`, `browser_hover`, `browser_drag`, `browser_drop`, `browser_press_key`, `browser_select_option`, `browser_fill_form`, `browser_file_upload`, `browser_handle_dialog`
  - Navigation: `browser_navigate`, `browser_navigate_back`, `browser_close`, `browser_resize`, `browser_tabs`
  - Inspection: `browser_snapshot`, `browser_take_screenshot`, `browser_console_messages`, `browser_network_requests`, `browser_network_request`
  - Network mocking: `browser_route`, `browser_route_list`, `browser_unroute`, `browser_network_state_set`
  - Cookies: `browser_cookie_clear`, `browser_cookie_delete`, `browser_cookie_get`, `browser_cookie_list`, `browser_cookie_set`
  - Misc: `browser_evaluate`, `browser_run_code_unsafe` (RCE-equivalent — explicitly labeled), `browser_wait_for`, `browser_get_config`
- **Concrete agent invocation.** MCP tool call (JSON-RPC `tools/call`); the agent picks `browser_*` tools the same way it picks any MCP tool. Refs come from `browser_snapshot` and feed back into `browser_click({ref: "..."})` etc.
- **Auth model.** Three modes: (1) **persistent profile** (default — like a real browser, cookies survive), (2) **isolated contexts** (each test gets a fresh context), (3) **connect to your existing browser** via the playwright-mcp browser extension. `--storage-state` flag for saved login state.
- **Multi-agent / parallel handling.** One MCP server per browser session by default. Multiple clients = run multiple `npx @playwright/mcp` processes. Standalone server mode for headless / worker scenarios.
- **Trust/safety.** `browser_run_code_unsafe` is explicitly labeled RCE-equivalent. Otherwise relies on Playwright's sandbox. No first-class prompt-injection scanner.
- **Structured output.** Accessibility tree (Playwright's a11y snapshot) is primary. Network requests numbered; agent fetches details on demand to avoid flooding context.
- **Integration shape.** Pure MCP server. Zero Rust. Adopting requires MCP client in hivecore (already on the v0.2 backlog).
- **License-vendor compatibility for hivecore.**
  - **Steal patterns: yes** — tool naming convention (`browser_*`), accessibility-tree-first stance, the "snapshot for actions, screenshot for confirmation" split.
  - **Steal code: not directly useful** — TS/Node, not Rust. Wrong runtime for our substrate.
  - **Best fit: alternative to native — orgs can plug playwright-mcp via our MCP layer if they prefer Playwright's wider browser matrix.**

## browser-use

- **URL.** https://github.com/browser-use/browser-use
- **License.** MIT. Compatible.
- **Language.** Python. 91.5k stars (largest by ~3 OOM).
- **Architecture.**
  - Python library: `from browser_use import Agent` then `Agent(task="...", llm=...).run()`.
  - Wraps Playwright internally; agent + browser bundled. The library *is* the agent — it owns the model loop, the tool dispatch, and the page interpretation.
  - Cloud offering for stealth / scale.
- **Tool surface.** Implicit — the Agent class drives Playwright. Action vocabulary is internal: navigate, click (by index from a numbered DOM extraction), input_text, scroll, extract_content, scroll_to_text, switch_tab, open_tab, etc. Tools are not externally addressable as MCP/CLI in the headline path.
- **Concrete agent invocation.** Python:
  ```python
  agent = Agent(task="Fill in this job application with my resume", llm=ChatBrowserUse())
  await agent.run()
  ```
- **Auth model.** Persistent context via Playwright; cookie injection at startup.
- **Multi-agent.** Cloud product runs many. OSS is single-agent per process.
- **Trust/safety.** No first-class injection guard.
- **Structured output.** DOM extraction with element indices; screenshots optional.
- **Integration shape.** A *whole agent* — wrong layer for hivecore. Useful as a competitor benchmark, not a dependency.
- **License-vendor compatibility.**
  - **Steal patterns: maybe** — index-based DOM extraction is older than ref-versioning; we should prefer refs.
  - **Steal code: no** — Python, agent-shaped, wrong abstraction layer for our `Tool`.

---

## Synthesis

Common patterns across all four:

1. **Accessibility tree > pixels.** Every modern impl (`gsd-browser`, `playwright-mcp`, `jcode`) leads with a structured snapshot (a11y tree + actionable list) and treats screenshots as visual confirmation only. browser-use is the only outlier (DOM extraction with indices) and that's the older pattern.
2. **Opaque element refs returned from snapshot, fed into actions.** Three different names — `@v1:e1` (gsd-browser, with explicit version), `el_2` (jcode/BPP), Playwright's accessibility ref (playwright-mcp) — same shape. Refs decouple "what the agent sees" from "what the page actually is" and survive minor selector churn.
3. **Re-snapshot after every navigation/DOM change is the agent's responsibility.** Stale refs are an invariant violation, not a soft warning. gsd-browser's SKILL.md and jcode's BPP both call this out explicitly.
4. **Daemon / persistent session split from CLI invocation.** Both Rust impls (gsd-browser, jcode) keep the browser alive in a daemon; each CLI/tool call is a thin attach. This is necessary for ref versioning to mean anything across calls.
5. **Provider/extension model so the browser flavour is swappable.** jcode's BPP is the most explicit (formal protocol, capability schema, multi-provider), gsd-browser and playwright-mcp are single-provider. The jcode pattern matches hivecore's "Layer 2 substrate, Layer 3 opinionated" thesis.

Divergences:

- **Skill vs MCP vs raw Tool.** gsd-browser ships a markdown skill + CLI. playwright-mcp is pure MCP. jcode is a single in-process `Tool` with action-string dispatch. All three are valid; the right one depends on whether the user wants the browser to run inside the harness process (jcode), as a separate daemon owned by the harness (gsd-browser), or as a wholly external MCP server (playwright-mcp).
- **Auth.** gsd-browser invented its own AES-GCM credential vault. playwright-mcp leans on Playwright's persistent context. jcode reuses the user's real Firefox profile via extension. browser-use side-steps with cookie injection. Each comes with a different trust story.
- **Prompt-injection defense.** Only gsd-browser has a first-class `check-injection` command. The others trust the agent + page-content boundary.

What hivecore should adopt vs avoid:

- **Adopt.** Accessibility-tree-first, versioned element refs, daemon model, provider/protocol abstraction (jcode's BPP), skill-as-front-door, `--json` everywhere.
- **Avoid.** Index-based DOM addressing (browser-use); single-monolithic-action with stringly-typed `action` enum (jcode's tool surface — it works but doesn't compose well with a typed `Tool` trait); RCE-labeled `*_unsafe` evals exposed by default (playwright-mcp does, gate it behind a capability flag).

## Recommendation for hivecore

1. **Layer-2 trait: `BrowserProvider`.** Steal jcode's BPP wholesale — namespace it `hivecore-browser-core` with `provider.describe`, `session.ensure/close`, `page.open/snapshot/click/type/wait/screenshot` as required, plus the optional groups. License-clean (MIT). Capability schema (`element_refs`, `a11y_snapshot`, `persistent_profile`, `isolated_contexts`, `attach_existing_browser`, etc.) maps directly onto our typed `Tool` surface.
2. **Layer-2 default provider: chromiumoxide-based daemon.** Mirror gsd-browser's architecture — single Rust binary, `chromiumoxide` for CDP, background daemon with per-session UNIX socket, ref-versioned snapshot. Vendor patterns (Apache-2.0; we already vendor Apache-2.0 prompts under ADR-024). Crate: `hivecore-browser-runtime`.
3. **Layer-3 tool surface: one typed `BrowserTool` per action, not a stringly-typed mega-tool.** Match the gsd-browser/playwright-mcp style (`browser_navigate`, `browser_snapshot`, `browser_click_ref`, ...) so each surfaces with its own JSON schema in the model's tool list. jcode's single-tool-with-action-enum is operationally fine but harder for models to use; the playwright-mcp split is the broader convention.
4. **Multi-tenant fit (our wedge).** `session_id` in BPP already carries provider-side isolation. Wire it to `tenant_id` at the tenancy plane (ADR-019): every `session.ensure` call must pass a tenant scope; provider must reject cross-tenant page handles. Per-tenant credential vault (steal gsd-browser's AES-GCM + Argon2 design) keyed by `tenant_id` is a natural extension.
5. **MCP escape hatch.** When v0.2 ships MCP support, expose `playwright-mcp` as an alternative provider behind the same `BrowserProvider` trait — gives orgs Firefox/WebKit coverage without us writing those drivers, and validates the protocol abstraction.

Action surface size for v0.1: start with the 12 BPP-required + ~8 most-used optionals (~20 actions). Match the jcode short-list, expand to gsd-browser's 63 over v0.2/v0.3 as real harness usage demands them. Avoid `eval` / `run_code_unsafe` until a capability gate exists.

---

## Sources cited

- gsd-browser README: https://github.com/gsd-build/gsd-browser/blob/main/README.md
- gsd-browser SKILL: https://github.com/gsd-build/gsd-browser/blob/main/SKILL.md
- gsd-browser Cargo.toml (chromiumoxide 0.9, aes-gcm, argon2): https://github.com/gsd-build/gsd-browser/blob/main/Cargo.toml
- jcode Browser Provider Protocol: https://github.com/1jehuang/jcode/blob/master/docs/BROWSER_PROVIDER_PROTOCOL.md
- jcode browser tool: https://github.com/1jehuang/jcode/blob/master/src/tool/browser.rs
- jcode browser provider plumbing: https://github.com/1jehuang/jcode/blob/master/src/browser.rs
- playwright-mcp README: https://github.com/microsoft/playwright-mcp/blob/main/README.md
- browser-use README: https://github.com/browser-use/browser-use/blob/main/README.md
