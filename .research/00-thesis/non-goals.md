# Non-goals

What hivecore is NOT, listed explicitly so scope drift is obvious.

## Not a coding agent

Hivecore does not generate code itself. It orchestrates coding agents (Claude Code, Codex, OpenCode, Pi, custom CLIs) via runtime adapters. The agents do the coding; hivecore provides the harness.

## Not a code editor / IDE replacement

Hivecore is not VS Code, Cursor, or JetBrains. The UI surfaces task management, run viewing, KG visualization, persona/workflow editing, skill registry, and admin — all SDLC orchestration concerns. Editing source code happens in your editor of choice; the agents handle most code edits inside sandboxes.

(**Note on prior scope decision:** an earlier draft excluded board+daemon as out-of-scope, intending to consume multica/paperclip. Revised: hivecore ships board+daemon natively. See ADR-014 for rationale. Multica/paperclip become competitors hivecore beats by offering the complete stack — KG-at-core + custom personas + custom workflows + control plane + native board + native daemon — under one OSS umbrella.)

## Not a hardcoded SDLC methodology

Spec-Kit, Kiro, BMad, OpenSpec hardcode the workflow. Hivecore lets orgs define their own DAG. We ship reference workflows as examples, not as the workflow.

## Not a vertical-specific tool

motadata-itsm is the **first vertical** for validation, not the product. The kernel is SDLC-general. Once kernel + first vertical prove out, other verticals (fintech, embedded, devops) plug in via their own ontology overlays.

## Not a SaaS-first product

OSS first. Self-hosted by default. Managed cloud may follow once kernel matures, never instead. Revenue (if any) is post-OSS-adoption, not gating it.

## Not a code knowledge graph product

PlayerZero, Bito, Tabnine ECE, Augment, Qodo, OneLens already do code KG. Hivecore's KG is part of the kernel — it serves persona context, gate inputs, skill ranking, audit trail. Generic code-KG is upstream we can ingest.

## Not a benchmark / evaluation framework

SWE-Bench Pro, Terminal-Bench, scenario benchmarks exist. Hivecore uses them as gates, doesn't compete.

## Not a model fine-tuning platform

Hivecore calls foundation models via runtime adapters. Training, fine-tuning, model hosting — out of scope. Use Anthropic, OpenAI, vLLM, etc.

## Not a chat UI

The frontend is for board, run viewer, KG visualizer, skill registry. Not for chatting with agents. Chat happens in the runtime CLIs (Claude Code, Codex, etc.) before they're invoked by hivecore.

## Not a CRM, project management tool, or ticketing system

Linear, Jira, GitHub Issues, ServiceNow already exist. Hivecore ingests from them and routes work to them; it doesn't replace them.

## Not a deployment platform

Kubernetes, Nomad, Terraform, Pulumi exist. Hivecore's deploy persona triggers existing tools; it doesn't reinvent them.

## Not commercial-only

Hivecore stays OSS. If a commercial managed offering emerges, it will run the same kernel that anyone can self-host.

## Not a closed ecosystem

Personas, workflows, skills are exchangeable across hivecore deployments (with org-private isolation preserved for sensitive ones). The community library is open.

## Not "everything for everyone"

Scope is **build-time SDLC harness**. Anything outside that scope (runtime ITSM, e-commerce, robotics, gaming AI, medical diagnosis) is explicitly out.
