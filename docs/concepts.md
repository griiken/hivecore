# Concepts

The shared vocabulary used throughout hivecore.

## Core nouns

### Persona
A declarative spec for a role an agent can play. Defines:
- name, description
- input artifact schema, output artifact schema
- tools allowlisted (MCP names)
- gates (computational + inferential) the persona must pass before emitting
- context loader (which KG subgraph slice to fetch)
- model preference + budget caps
- retry policy

Personas are CONFIG, not code. Lives in `.hivecore/personas/<name>.toml`.

### Workflow
A directed acyclic graph of state transitions. Each node = a persona invocation. Edges have transition guards (verdicts, gates, human approval rules). Workflows are CONFIG (`.hivecore/workflows/<name>.dag.toml`), not hardcoded.

### Control plane
The governance layer: RBAC, policies, budgets, audit, kill switches, escalation rules. All configurable per workspace, per workflow, per persona.

### Knowledge Core
A multi-tenant isolated knowledge graph instance (TrustGraph pattern). Each tenant has its own core; cores are swappable, versioned, permissioned. Cross-core queries forbidden by default.

### Subgraph
A typed slice of a Knowledge Core scoped to a domain or aspect:
- **Code** — files, functions, classes, modules, calls, imports, defines
- **SDLC** — issues, PRs, sprints, deployments
- **Decision** — ADRs, design docs, rationale spine
- **Test** — test cases, coverage, mutation scores
- **Runtime** — telemetry traces, error events, code-symbol links
- **Customer** — per-customer customizations (in vertical deployments)
- **Domain** — vertical-specific entities (e.g. SlaPolicy, Plugin in ITSM)
- **Skill** — reusable playbooks with provenance and success rates
- **Run** — agent execution traces, append-only event log
- **Tribal** — Slack threads, support tickets, design discussions

### Bi-temporal fact
Every edge in the KG has two time axes:
- **valid_time** — when the fact was true in the world
- **transaction_time** — when the system learned it
Lets agents query "what did we believe at point T" vs "what is true now."

### Skill
A versioned, signed, reusable playbook (markdown + optional scripts) that a persona invokes. Tracks provenance (which PRs it produced, success rate per entity type).

### Run
One execution attempt of a workflow node by a persona. Produces an artifact + verdict. Append-only event log (every prompt, tool call, file edit, gate result) → enables replay.

### Sandbox
An ephemeral isolated execution environment per run. Firecracker microVM by default; Docker for less-sensitive contexts. Network egress allowlist, secrets via short-TTL token broker, resource quotas enforced.

### Gate
A pre-emit verification step on persona output. Two classes:
- **Computational** — fast, deterministic (build, lint, typecheck, schema-validate, test, codemod, structural lint, GNN vuln scan).
- **Inferential** — slow, probabilistic (LLM-as-judge, semantic review).
Gates run in series; inferential only after computational passes.

### Scenario
An executable validation case: input intent, expected behavior, assertions. Replaces "is this code good?" with "does it satisfy these scenarios?". Stored in `scenarios/` per repo.

### Digital twin
A mock of a dependent external system used during scenario execution (e.g., a customer's prod request stream replayed; a stub of an external API). Lets agents validate end-to-end without touching production.

### Verdict
The output of a persona run: `PASS`, `FAIL`, `ESCALATE_HUMAN`, `RETRY`. Drives workflow state machine transitions.

### Repo-as-harness
Pattern where a target repo carries its own SDLC harness configuration in `.hivecore/`: persona overrides, workflow choice, scenarios, skills, run state. Externalizes context to the repo, not the prompt.

## Auxiliary nouns

### Coordination board
A shared persistent state that tracks file ownership across concurrent agent runs. Prevents merge conflicts at the filesystem level.

### Message bus
Async inter-agent communication channel. Agents emit messages; orchestrator routes by topic + recipient persona.

### Episode
A discrete event captured into the KG at the lowest tier (Graphiti pattern). Episodes get extracted into entities, then clustered into communities.
