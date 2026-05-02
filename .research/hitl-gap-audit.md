# HITL gap audit — six unsurveyed OSS coding agents

**Date:** 2026-05-03 · **Scope:** Aider, Cline, Continue.dev, Goose, OpenHands SDK, Claude Code.
**Q:** Did `Approval`/`ApprovalHook` (ADR-029) miss load-bearing design moves?
**Baseline:** `.research/hitl-prior-art.md` (pi/Codex/ACP/Warp); code at `crates/hivecore-runtime-core/src/approval.rs` + `crates/hivecore-approval-policy/`.

## Hivecore today (no re-survey)

- Trait (`approval.rs:25-112`): `ApprovalAction { Tool | ApplyPatch | Mcp }`, `ApprovalDecision { Approved | ApprovedForSession | ApprovedAndPersist{rule} | Denied | TimedOut | Abort | Cancelled }`, `ApprovalScope { Call | Turn | Session }`, `RiskHint { read_only, risky, network }`, `ApprovalRule { ToolPrefix | McpToolAllow | PathRoot { write } }`.
- Policy dial (`policy.rs:9-18`): `OnRequest | UnlessTrusted | Never`.
- Matcher (`matcher.rs:19-46`): single `ToolMatcher` trait; default = static deny-list by name (`coder_defaults` denies `bash`/`write_file`/`edit_file`).
- Cache (`cache.rs:14-43`): in-memory `HashSet<(name, sha256(canonical_input))>`. Persistent `ApprovedAndPersist` deferred to v0.2.
- Hook (`hook.rs`): adapts `Approval` into `ToolHook` via `HookOutcome`.

---

## 1. Aider (Aider-AI/aider, MIT)

**State machine.** Single `confirm_ask` (`aider/io.py:807-919`). Options `(Y)/(N)/(A)ll/(S)kip all/(D)on't ask again`. The `(A)ll`/`(S)kip all` are scoped to a transient `ConfirmGroup` so a batch of related questions shares one decision (`io.py:870-872, 916-921`). Group preference is in-memory.

**Persistence.** `(D)on't ask again` → `self.never_prompts: set[(question, subject)]` (`io.py:823-824, 902-903`). **Session-only, never written to disk.** Append into chat history is transcript-only.

**Modes.** `--yes-always` (`args.py:759-763`) blanket-bypass; `--auto-commits` / `--auto-lint` (default true), `--auto-test` (default false) auto-approve post-edit gates not tool calls. `explicit_yes_required=True` (`io.py:812, 908`) forces literal `y`, treats Enter as `n` — used for destructive ops (URL open, deletion).

**Patterns hivecore lacks.**
- **`ConfirmGroup` (batch consent).** Between `Call` and `Turn`. Concretely useful for multi-file patches; pi has nothing similar; Codex `Turn` is the closest.
- **`explicit_yes_required` field on the request.** Flips the default key behaviour — destructive ops require literal y. Two-line addition.

---

## 2. Cline (cline/cline, Apache-2.0)

**Settings type** (`src/shared/AutoApprovalSettings.ts:1-26`). Per-tool toggles: `readFiles`, `readFilesExternally`, `editFiles`, `editFilesExternally`, `executeSafeCommands`, `executeAllCommands`, `useBrowser`, `useMcp`. `maxRequests`/`enabled`/`favorites` are legacy (kept for back-compat). `enableNotifications` global.

**State machine.** `AutoApprove.shouldAutoApproveTool` (`src/core/task/tools/autoApprove.ts:42-117`) returns `boolean | [boolean, boolean]` — tuple `[localFlag, externalFlag]` for inside-vs-outside workspace root. `shouldAutoApproveToolWithPath` (`autoApprove.ts:122-167`) resolves the path, calls `isLocatedInWorkspace`, **demands BOTH `local` AND `external` toggles for external paths** (`autoApprove.ts:163`).

**Override stack.** Two master switches checked first: `yoloModeToggled` and `autoApproveAllToggled` (`autoApprove.ts:43-86`). Per-MCP-tool toggles via `toggleToolAutoApproveRPC(serverName, toolNames, autoApprove)` (`src/core/controller/mcp/toggleToolAutoApprove.ts:13-18`) persisted in MCP server config.

**Patterns hivecore lacks.**
- **Workspace-root vs external path scope.** Critical for repo-as-harness — auto-approve `crates/foo/`, prompt on `~/.ssh/`. Add as `RiskHint.path_scope` or separate.
- **Per-(MCP-server, tool) auto-approve toggle on the server-config side.** Hivecore's `ApprovalRule::McpToolAllow` is a rule but isn't a UI toggle on the server registry.
- **Two-bit shell safety split** (`executeSafeCommands` vs `executeAllCommands`). Implies a `terminal-security` classifier we do not have.

---

## 3. Continue.dev (continuedev/continue, Apache-2.0)

**Permission types.** `allow | ask | exclude` (`extensions/cli/spec/permissions.md:5-9`). **`exclude` removes the tool from the model's tool list entirely** — third state hivecore lacks.

**Precedence stack** (5 levels, earlier wins, `spec/permissions.md:11-19`):
1. Mode policies (`plan` / `auto`)
2. CLI flags (`--allow`, `--ask`, `--exclude`)
3. `config.yaml` permissions
4. `~/.continue/permissions.yaml`
5. Default policies

**Pattern syntax.** `parseToolPattern` (`permissionsYamlLoader.ts:77-117`): `Read`, `Read(*)`, `Read(**/*.ts)`, `Bash(npm install)`. Per-tool primary-arg map (`toolArgMappings`: `Write→file_path`, `Bash→command`, `Fetch→url`); `Read(./secrets/**)` becomes `argumentMatches: { file_path: "./secrets/**" }`. Glob `*`/`?` escaped to regex (`permissionChecker.ts:51-58, 79-94`).

**Dynamic policy hook** (`permissionChecker.ts:151-167`). `evaluateToolCallPolicy(basePolicy, args)` runs after static matching. **Safety property: dynamic eval can only DOWNGRADE to `disabled`, never upgrade.** Comment `permissionChecker.ts:162` — "If dynamic evaluation says disabled, that ALWAYS takes precedence … Otherwise, user preference wins." Defense-in-depth.

**Mode policies** (`defaultPolicies.ts:42-71`). `PLAN_MODE_POLICIES` excludes Edit/Write/MultiEdit; `AUTO_MODE_POLICIES = [{tool:"*", permission:"allow"}]`. Total override.

**Headless mode** (`spec/permissions.md:92-112`). In `-p`/`--print` mode, `ask` does NOT prompt — **process exits with error**. Forces explicit pre-approval.

**Patterns hivecore lacks.**
- **`exclude` as third level** — model never sees the tool.
- **Argument-glob match at rule level** (`Read(./secrets/**)`).
- **5-layer precedence stack** (mode > CLI > project config > user config > defaults). Hivecore is single-layer.
- **Headless-mode "ask = error".** Today an `Approval` sink in CI would hang forever.
- **Mode as a clean total-override layer** that can both deny tools AND auto-approve, not just `Never` matcher-bypass.
- **Downgrade-only dynamic hook.** Tool-side veto that can't be elevated.

---

## 4. Goose (block/goose, Apache-2.0)

Deepest design surface of the six.

**Mode enum.** `GooseMode { Auto | Approve | SmartApprove | Chat }` (`crates/goose/src/agents/agent.rs:222, 390-394, 1413`). **Mutable mid-session** via `update_goose_mode(mode, session_id)` (`agent.rs:1882-1897`) and **persisted to the session record** so it survives resume.

**Persistence.** `~/.config/goose/permission.yaml` (`crates/goose/src/config/permission.rs:11`). Schema: `HashMap<String, PermissionConfig>`, outer key is *category* (`user` or `smart_approve` — `permission.rs:41-42`). Each `PermissionConfig` has three lists: `always_allow`, `ask_before`, `never_allow` (`permission.rs:26-31`). **Two-tier table — `user` overrides `smart_approve`.** Updates atomically move tools across lists (`permission.rs:119-144, 183-214`). Extension-uninstall purges by prefix (`permission.rs:217-234`).

**`smart_approve` = LLM-as-judge for read-only classification.** `permission_judge.rs:122-154` (`detect_read_only_tools`) sends pending tool requests + `permission_judge.md` system prompt to the model with tool `platform__tool_by_tool_permission` returning a list of read-only tool names. Independent fast path: `apply_tool_annotations` (`permission.rs:101-117`) consumes **MCP `ToolAnnotations.read_only_hint`** flag and bulk-pre-populates `ask_before` for write-annotated tools so the LLM judge isn't invoked for them.

**Decision-side enum.** `Permission { AlwaysAllow | AllowOnce | Cancel | DenyOnce | AlwaysDeny }` (`permission_confirmation.rs:6-12`). **Five variants — hivecore lacks `AlwaysDeny` (persistent deny).** `PrincipalType { Extension | Tool }` (`permission_confirmation.rs:15-18`) lets the user lift a decision from one tool to all tools in an extension — principal hierarchy.

**Tool inspection pipeline** (`agent.rs:280-303`). Five inspectors run as a stack before any approval check:
1. `SecurityInspector` — pattern matcher (highest priority)
2. `EgressInspector` — network egress checks
3. `AdversaryInspector` — opt-in LLM-based review enabled by `~/.config/goose/adversary.md`
4. `PermissionInspector` — consumes the YAML config above
5. `RepetitionInspector` — flags tool-call loops

Each inspector is independent and can flag a call. Hivecore's single `ToolMatcher` is monolithic.

**Patterns hivecore lacks.**
- **`smart_approve` LLM-judge for runtime read-only classification** when `RiskHint` is unset (`permission_judge.rs:19-65`). Today `RiskHint=None` → user prompted for `git status`.
- **MCP `ToolAnnotations.read_only_hint` consumption** (`permission.rs:101-117`). Standard MCP annotation. `hivecore-mcp-client` does not wire this. Goose proves ~20-line wiring.
- **`AlwaysDeny` persistent decision variant.** Mirror to `ApprovedAndPersist`.
- **`PrincipalType` (Extension vs Tool).** Lifts decisions a level — "deny *every* tool from this MCP server."
- **Inspector pipeline architecture.** `Vec<Box<dyn ToolInspector>>` of single-responsibility classifiers. More extensible than one `ToolMatcher`.
- **Two-tier permission table (`user` overrides `smart_approve`).** User manual locks survive even if LLM judge later flags a tool.
- **Mode mutability mid-session, journaled to session record.** Hivecore `ApprovalPolicy` is fixed at construction.
- **Repetition detection as approval trigger.** `RepetitionInspector` catches LLM stuck in tool-call retry loop.

---

## 5. OpenHands SDK (OpenHands/software-agent-sdk, MIT)

Repo renamed: `All-Hands-AI/OpenHands` → `OpenHands/OpenHands` + new SDK at `OpenHands/software-agent-sdk`. Confirmation/security logic now in the SDK.

**Risk model.** `SecurityRisk { UNKNOWN, LOW, MEDIUM, HIGH }` (`openhands-sdk/openhands/sdk/security/risk.py:13-23`) with strict ordering. **`UNKNOWN` is not comparable** — comparisons raise `ValueError` (`risk.py:95`). Each level has `description` and Rich-text `visualize` (`risk.py:25-67`) so it's first-class for UI.

**ConfirmationPolicy hierarchy** (`confirmation_policy.py:1-62`). Three impls of `ConfirmationPolicyBase`:
- `AlwaysConfirm` — every action.
- `NeverConfirm`.
- `ConfirmRisky { threshold: SecurityRisk = HIGH, confirm_unknown: bool = True }` — fires when `risk.is_riskier(threshold)` (reflexive: `HIGH >= HIGH` triggers).

Richer than hivecore's `ApprovalPolicy { OnRequest, UnlessTrusted, Never }`: threshold parametric, `confirm_unknown` independent. Hivecore's `UnlessTrusted` collapses these two.

**SecurityAnalyzer pipeline** (`analyzer.py:15-39`). Trait: `security_risk(action) -> SecurityRisk`. Concrete:
- `LLMSecurityAnalyzer` (`llm_analyzer.py:10-29`) — trusts a `security_risk` field set by the LLM in the action emit. **The LLM self-reports its own risk** as part of structured action output. Cheapest analyzer.
- `EnsembleSecurityAnalyzer` (`ensemble.py:22-101`) — wires multiple analyzers, **max-severity fusion** with `propagate_unknown` mode (any UNKNOWN propagates) vs default (filter UNKNOWN, max of concrete). **`fail-closed to HIGH` on analyzer exception** (`ensemble.py:84-86`).
- `defense_in_depth/policy_rails.py` — composed-threat detection.
- `defense_in_depth/pattern.py` — known-signature matcher.
- `grayswan/analyzer.py` — third-party adversarial classifier (vendor plug-in at trait level).

**Patterns hivecore lacks.**
- **Parametric risk threshold.** `ConfirmRisky(threshold=MEDIUM, confirm_unknown=True)`. Replace closed `ApprovalPolicy` with struct.
- **`UNKNOWN` as first-class incomparable variant.** Hivecore's `Option<bool>` silently treats `None` as false. OpenHands raises — forces policy to confront uncertainty.
- **LLM-self-reported risk on action emit.** Different from emitter-side `RiskHint`: model introspects on its own action. Cheap, lyable, needs ensemble.
- **Ensemble fusion with explicit fail-closed-to-HIGH.** No fusion concept in hivecore.
- **Pluggable third-party adversarial classifier** at the analyzer-trait level.

---

## 6. Claude Code (closed source, docs at docs.claude.com/en/docs/claude-code/settings)

Fetched 2026-05-03 (2.0 MB rendered HTML, single page).

**Permission rule schema.** `permissions: { allow: [...], deny: [...], ask: [...] }`. **Three lists** — hivecore has only two effective states (matched-by-deny-list = ask, otherwise = allow). Rule strings: `Bash(npm run lint)`, `Bash(npm run test *)`, `Bash(curl *)`, `Read(~/.zshrc)` (tilde-expanded), `Read(./.env)`, `Read(./.env.*)`, `Read(./secrets/**)`.

**Permission modes.** `defaultMode`: `default | acceptEdits | plan | auto | dontAsk | bypassPermissions`. Six modes:
- `default` — prompt on write, allow on read
- `acceptEdits` — auto-approve edits, still prompt on bash
- `plan` — read-only (no edits, no bash mutations)
- `auto` — auto-approve everything
- `dontAsk` — between `acceptEdits` and `auto`
- `bypassPermissions` — full disable, requires `--dangerously-skip-permissions`

**Multi-scope settings.** Four scopes, precedence top-down:
- **Managed** — `managed-settings.json`, plist/registry, deployed by IT
- **User** — `~/.claude/`
- **Project** — `.claude/` (git-committed)
- **Local** — `.claude/settings.local.json` (gitignored)

**Enterprise lockout primitives.**
- `disableBypassPermissionsMode: "disable"` — kills the `--dangerously-skip-permissions` flag
- `*RulesOnly` — managed-only setting that prevents user/project scopes from defining `allow`/`ask`/`deny` rules; only managed rules apply
- `additionalDirectories` — explicit allow-list of extra working dirs outside the project root

**Patterns hivecore lacks.**
- **Three-state rule list (`allow`/`deny`/`ask`).** Hard `deny` (deterministic reject without prompting) ≠ "always-ask-then-deny." Faster, fatigue-resistant, stable error surface.
- **6-mode permission state machine.** `acceptEdits` (auto-write but ask-bash) and `plan` (read-only) don't compose from `OnRequest/UnlessTrusted/Never`.
- **4-scope settings hierarchy** (managed > user > project > local). `.claude/settings.local.json` (gitignored) for personal overrides + `.claude/settings.json` (committed) for team policy is the policy-as-code shape that maps onto repo-as-harness.
- **Managed-only enforcement primitives** (`*RulesOnly`, `disableBypassPermissionsMode`). Enterprise lockout — IT-level scope cannot be loosened by lower scopes.
- **`additionalDirectories` allow-list** of trusted external roots.
- **Explicit signed warning gate** for bypass (`--dangerously-skip-permissions`) plus its lockout via managed config.

---

## Synthesis: adopt now / adopt v0.2 / defer / reject

### Adopt now (low cost, fixes real v0.1 gaps)

| # | Pattern | Source | Hivecore change |
|---|---------|--------|-----------------|
| A1 | **Add `Deny` to matcher result; matcher returns `Decision::{Ask, Deny, Allow}` enum** instead of `Option<reason>` | Claude Code, Goose | Symmetry with `Allow`. Today a deny-listed tool always prompts → fatigue attack. |
| A2 | **`exclude`/disabled state — drop the tool from the model's tool list** | Continue.dev | New driver-side filter at tool-list assembly. Defense-in-depth: model can't try what it can't see. |
| A3 | **Consume MCP `ToolAnnotations.read_only_hint`** | Goose, MCP spec | `hivecore-mcp-client` reads annotation → populates `RiskHint.read_only`. Closes "user prompted for `git status`" without LLM judge. ~20 lines. |
| A4 | **Group/batch scope between `Call` and `Turn`** | Aider | Add `ApprovalScope::Group(GroupId)` + `GroupId` on `ApprovalRequest`; sink can return `ApprovedForGroup`. Real win for multi-file patches. |
| A5 | **Headless / non-interactive policy: `FailOnAsk`** | Continue.dev | Add variant. Today an `Approval` sink in CI hangs forever. Land before wiring ACP. |
| A6 | **`explicit_yes_required` field on `ApprovalRequest`** | Aider | Bool flag set by emitter for destructive ops; sink requires literal-yes (not Enter-default). Two-line addition. |

### Adopt as v0.2 audit/policy plane lands

| # | Pattern | Source |
|---|---------|--------|
| B1 | **4-scope settings hierarchy: managed > user > project > local.** `.hivecore/permissions.toml` (committed) + `.hivecore/permissions.local.toml` (gitignored). Managed for verticals later. | Claude Code |
| B2 | **Inspector pipeline (`Vec<Box<dyn ToolInspector>>`).** Replaces the monolithic `ToolMatcher`. Each inspector unit-tested (security, egress, repetition, MCP-annotation, LLM judge). Composes with ensemble fusion. | Goose, OpenHands |
| B3 | **Argument-glob match at rule level** (`Bash(npm run test *)`, `Read(./secrets/**)`). Per-tool primary-arg mapping. | Continue.dev, Claude Code |
| B4 | **`PrincipalType` (Server vs Tool) for MCP grants.** Lift `McpToolAllow` to `McpAllow::{Server, Tool}` — deny entire MCP server in one decision. | Goose |
| B5 | **Persistent `AlwaysDeny` decision variant.** Symmetric with `ApprovedAndPersist`. | Goose |
| B6 | **Mode mutability mid-session, journaled.** Hivecore already has JSONL per ADR-026; add `mode_change` event variant. | Goose |
| B7 | **Workspace-root vs external path scope.** `RiskHint.path_scope: WorkspaceLocal | External | None`. Auto-allow workspace-local under `UnlessTrusted`. | Cline |
| B8 | **Risk threshold parameterization** (`ConfirmRisky(threshold=MEDIUM, confirm_unknown=true)`). Replace closed enum with struct: `{ require_above, on_unknown }`. | OpenHands |
| B9 | **Repetition detection as approval trigger.** Inspector that escalates after N identical calls. | Goose |

### Defer to vertical rollouts

| # | Pattern | Source |
|---|---------|--------|
| C1 | LLM-judge `smart_approve` for runtime read-only classification. Latency + cost tax; wire only after B2 + A3 exhausted. | Goose |
| C2 | Ensemble analyzer (max-severity fusion + `propagate_unknown` + fail-closed-to-HIGH). Implementation follows directly once B2 lands. | OpenHands |
| C3 | Defense-in-depth named layers (Pattern, PolicyRail, GraySwan). Map onto hivecore inspectors but require curated signature lists. | OpenHands |
| C4 | Managed-only enforcement primitives (`*RulesOnly`, `disableBypassPermissionsMode`). Needed when a vertical deploys to IT-compliant customer. | Claude Code |
| C5 | LLM-self-reported risk on action emit. Needs structured action emit aligned with model output schema. Defer until ACP action vocabulary lands. | OpenHands |

### Reject

| # | Pattern | Source | Reason |
|---|---------|--------|--------|
| R1 | Six-mode flat enum | Claude Code | Better as struct with orthogonal axes (B8). The flat enum is a UX shape, not architecture. |
| R2 | Cline `executeSafeCommands` vs `executeAllCommands` two-bit shell split | Cline | Classifier (`@continuedev/terminal-security`) is a project of its own. Use B2 inspector pipeline + future `BashSafetyInspector` plugin. |
| R3 | `yoloModeToggled` master switch | Cline | `ApprovalPolicy::Never` already exists. Adding a redundant switch invites drift. |
| R4 | Aider's session-only-set `(D)on't ask again` | Aider | Bug, not design. We already plan persistent `ApprovedAndPersist` via audit plane. |

---

## ADR-029 implications

This audit reveals the **policy plane** is undersized:

1. Matcher is a single trait when it should be an inspector pipeline (B2).
2. Decision space is `Option<reason>` when it should be `Decision::{Ask, Deny, Allow, Exclude}` (A1, A2).
3. Risk model is `Option<bool>` flags when it should be a `SecurityRisk` ordered enum with explicit `Unknown` (B8).
4. No settings file (B1), no scope hierarchy (B1), no mid-session mode mutation (B6).

**Recommendation: revise ADR-029 to land A1–A6 before wiring ACP.** A1, A2, A5, A6 change the trait surface — landing them after ACP wiring forces a second round of breaking changes on the wire protocol. A3 and A4 are additive.

B1–B9 = ADR-029 v2 / v0.2 audit-policy plane, with B2 (inspector pipeline) as the anchor decision since the others compose into it.

C1–C5 deferred until vertical deployments demand them.

---

## Sources

- Aider: `aider/io.py:807-919`, `aider/args.py:439-557, 759-763` (https://raw.githubusercontent.com/Aider-AI/aider/main/aider/).
- Cline: `src/shared/AutoApprovalSettings.ts:1-44`, `src/core/task/tools/autoApprove.ts:42-167`, `src/core/controller/mcp/toggleToolAutoApprove.ts:13-25` (https://raw.githubusercontent.com/cline/cline/main/).
- Continue.dev: `extensions/cli/src/permissions/{defaultPolicies.ts:1-72, permissionChecker.ts:17-181, permissionsYamlLoader.ts:11-179}`, `extensions/cli/spec/permissions.md:1-113` (https://raw.githubusercontent.com/continuedev/continue/main/).
- Goose: `crates/goose/src/config/permission.rs:11-235`, `permission/permission_confirmation.rs:4-25`, `permission/permission_judge.rs:19-154`, `agents/agent.rs:222-303, 390-394, 1882-1901` (https://raw.githubusercontent.com/block/goose/main/).
- OpenHands SDK: `openhands-sdk/openhands/sdk/security/{risk.py:10-148, confirmation_policy.py:9-62, analyzer.py:15-112, llm_analyzer.py:10-29, ensemble.py:22-101}` (https://raw.githubusercontent.com/OpenHands/software-agent-sdk/main/).
- Claude Code: https://docs.claude.com/en/docs/claude-code/settings (fetched 2026-05-03) — `permissions.allow/deny/ask`, `defaultMode`, scope hierarchy, `disableBypassPermissionsMode`, `*RulesOnly`.
