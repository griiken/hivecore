# crates/hivecore-browser-core/AGENTS.md

**Layer 2.** Pure `BrowserProvider` trait + types. No I/O dependencies. ADR-028.

Lifted from jcode's `Browser Provider Protocol` (MIT — see ADR-024 vendoring rules).

## What lives here

| Module | Purpose |
|---|---|
| `provider` | `BrowserProvider` async trait — required + optional methods. |
| `context` | `TenantId`, `BrowserSessionId`, `BrowserContext`. Multi-tenant scope. |
| `snapshot` | `Snapshot`, `SnapshotElement`, `SnapshotMeta`, `ElementRef`, `SnapshotVersion`. |
| `actions` | `ActionKind`, `ActionOutcome`, `WaitCondition`, `AssertPredicate`. |
| `capability` | `Capability` enum + `CapabilityTable` with stability tiers. |
| `error` | `BrowserError` (`thiserror`). |

## Load-bearing invariants

1. **No I/O** — pure types only. Concrete impls live in sibling crates.
2. **Multi-tenant by construction** — `BrowserContext { tenant, session }` flows
   through every BPP call; provider must reject cross-tenant access.
3. **Refs versioned + stale-on-navigation** — `ElementRef { version, element_id }`.
   Old refs invalid after the next snapshot bumps the version.
4. **`Snapshot::lookup` rejects mismatched versions** — type-level enforcement
   that callers re-snapshot.

## Tests

```
cargo test -p hivecore-browser-core
```

3 unit tests cover ref roundtrip, parse rejection, stale-version lookup.

## v0.2 backlog

- Promote `TenantId` from a `String`-newtype here to a Layer-1 newtype in
  `hivecore-runtime-core::ids` once the Tenancy plane lands (ADR-029 planned).
- `WaitCondition::SelectorVisible` / `RefVisible` / `UrlMatches` — currently
  defined but not exercised; runtime falls back to delay-with-timeout. Wire
  full predicate evaluation in providers.
- `BrowserHook` trait (sensitive-action gate) — analog of `ToolHook`. Fires
  before credential entry / payment / CAPTCHA / file upload-download.
- `StorageState` capability + serialiser/deserialiser (Playwright shape).
- `VisionFallback` capability — `browser_scene_understand` /
  `browser_vision_click` types belong here once vision-delegate trait lands.

## Stability

`BrowserProvider` trait shape is the load-bearing API. Treat as semver-binding
once any external impl lands. `Capability` enum is non-exhaustive — additive
new variants don't break consumers.
