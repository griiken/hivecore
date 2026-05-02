# End goal

## Mission

Make AI-driven SDLC harnesses **adoptable, customizable, and ownable** by any organization — without locking them into a vendor's opinion of what their personas, workflow, or governance should look like.

## North star

Five years out:

- Hundreds of organizations run hivecore as their SDLC harness substrate.
- Each has its own persona library, workflow DAG, and control plane — none looking exactly alike.
- Skills compound across the community: a fintech-specific compliance-officer persona authored at one org gets shared, reviewed, and reused at others (with org-private isolation preserved).
- "Validation replaces review" becomes the dominant pattern: agents author code, scenarios validate it against digital-twin mocks of dependent systems, humans intervene only at policy boundaries.
- Hivecore's KG layer becomes the de-facto open standard for SDLC knowledge graphs, ingested by agents from any vendor.

## Non-goals

- Not a coding agent (those are runtime adapters underneath).
- Not a board+daemon platform (multica/paperclip do that — hivecore consumes them).
- Not a hardcoded SDLC methodology.
- Not a vertical-specific tool.
- Not a SaaS-first product. OSS first; self-hosted by default. Managed cloud may follow, never instead.

## Strategic posture

### Build-time vs runtime

Hivecore targets **build-time customization authoring** (writing code, plugins, queries, schemas, configs). It does NOT target runtime ticket resolution, end-user automation, or production traffic handling.

This positions hivecore upstream of platforms like Ivanti, Aisera, Wolken (ITSM runtime), ServiceNow (runtime), Salesforce Einstein (runtime). Different segment, no direct collision.

### OSS-first economics

Hivecore is licensed dual MIT-OR-Apache-2.0. Revenue, if any, comes from optional managed cloud later — never from gating core features.

The bet: OSS adoption is the moat. Internal AI software factories proved the economics ($X engineering hours saved per merged PR); hivecore makes that achievable by orgs that can't afford to build their own. Adoption velocity > extraction velocity.

### Standards posture

Hivecore aims to be a *reference implementation*, not a sealed product. The KG ontology, persona schema, workflow DAG syntax, and control-plane policy format will be specified openly, so other implementations can interoperate.

## Validation

End goal is achieved when:

- ≥10 unrelated organizations contribute personas/workflows back to a shared community library.
- Hivecore's KG ontology gets adopted (by extension or import) by ≥3 other agent platforms.
- Public conferences treat "hivecore-compatible" as a category descriptor.
- The substrate is stable enough that orgs run multi-year deployments without breaking changes.

## Timeline

No dates. Phases gated by dependency, not calendar (see [docs/roadmap.md](../../docs/roadmap.md)).

The current binding constraint is the kernel itself: P0 → P5 (KG substrate). Everything else compounds from there.
