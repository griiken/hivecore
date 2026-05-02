# Security

Public-facing security guidance for hivecore operators. For vulnerability reports, see root [SECURITY.md](../SECURITY.md).

## Threat model

Hivecore runs autonomous agents that read code, modify files, execute commands, and open pull requests. The threats are:

1. **Sandbox escape** — agent executes code that breaks out of the sandbox, accesses host or other tenants' data.
2. **Cross-tenant data leakage** — Knowledge Core for tenant A leaks facts into tenant B's queries.
3. **Secret exfiltration** — agent reads secrets from environment, log lines, or repo history and writes them to PR bodies, external APIs, or chat.
4. **Prompt injection** — malicious content in ingested data (tickets, PR comments, code comments) hijacks agent behavior.
5. **Agent runaway** — agent loops indefinitely, exhausts budget, fills storage, opens spam PRs.
6. **Tool misuse** — agent invokes a tool with excessive privileges (e.g., deletes branches, force-pushes).
7. **Supply chain** — compromised dependency in adapter or skill leaks credentials or runs unauthorized code.

## Mitigations (built into hivecore)

### Sandbox isolation
- Firecracker microVMs by default; one VM per run.
- Read-only base FS; scratch overlay only.
- Network egress allowlist per task; no open internet.
- Resource quotas (CPU/memory/disk/wallclock).
- Sandbox destroyed at end of run; nothing persists.

### Multi-tenant isolation
- Each tenant = separate Knowledge Core instance.
- Permission-aware retrieval at query layer, not at prompt layer.
- Cross-tenant queries forbidden at API; rejected before reaching agent.
- Audit log per tenant, hash-chained, append-only.

### Secrets handling
- Short-TTL token broker; agents never see long-lived secrets.
- Tokens scoped to single repo + single run.
- PR bodies, log lines, and KG writebacks scrubbed for secret patterns before persistence.
- Env vars never injected into agent context unless explicitly allowlisted in policy.

### Prompt injection defense
- Untrusted content (tickets, PR comments, code comments from external contributors) marked with provenance tags.
- Agents instructed to ignore instructions inside untrusted content.
- Critical actions (merge, deploy, push to main) require either a human approval gate or a hash-verified policy assertion.

### Runaway control
- Hard budget cap per run (tokens, time, dollars).
- Soft warning at 80%; hard kill at 100%.
- Per-team and per-skill rollup budgets.
- Loop detection: if agent emits same tool call N times within window, kill run.

### Tool allowlisting
- Each persona has explicit `tools` allowlist in its TOML config.
- Tools outside the list are unavailable to that persona.
- Destructive tools (force-push, branch delete, db drop) require an additional policy approval gate.

### Supply chain
- Skills are signed and versioned in the registry.
- Skill provenance recorded in KG; signed-off-by required for skill publication.
- Adapter crates pinned by version + checksum.
- CI runs `cargo audit` and `pnpm audit` on every PR.

## Operator responsibilities

When deploying hivecore in your org:

1. **Bring your own LLM API keys** — store in a secret manager (Vault, AWS Secrets Manager), inject via the broker pattern.
2. **Configure egress allowlists** per team and per repo.
3. **Set realistic budgets** — start strict, loosen as patterns emerge.
4. **Enable audit log retention** — at least 90 days for compliance debugging.
5. **Review skill publications** — treat skill PRs like code PRs, not docs.
6. **Rotate sandbox base images** monthly.
7. **Monitor for prompt injection patterns** — log untrusted-content provenance and review periodically.

## Compliance

Hivecore architecture is designed-in for SOC 2 Type II posture (audit log, RBAC, secret broker, isolation, change tracking). Certification is deferred until commercial launch but the substrate supports it.

For HIPAA / PCI / FedRAMP, additional hardening is required (deployment topology, encryption-at-rest key management, access reviews). Hivecore can support these but does not certify them.

## Reporting

See [SECURITY.md](../SECURITY.md) for vulnerability reporting.
