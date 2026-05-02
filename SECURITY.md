# Security policy

## Reporting a vulnerability

Email **dipendra497@gmail.com** with subject `SECURITY: hivecore <short summary>`. Do not open a public issue for security reports.

We aim to acknowledge within **3 business days** and provide a fix or coordinated disclosure timeline within **30 days** for confirmed issues.

## Scope

Security-relevant components in hivecore:

- Sandbox isolation (Firecracker / Docker runner)
- Secret broker + token issuance
- Multi-tenant Knowledge Core isolation (cross-tenant data leakage)
- Permission policies + RBAC
- Agent tool allowlists
- Audit log integrity (hash-chained append-only)
- Webhook signature verification
- Authentication and session management

## Out of scope (for now)

- Self-hosted deployment misconfigurations (covered by docs/security.md best practices)
- Third-party service integrations (route to their security teams)

## Disclosure

We coordinate disclosure with the reporter. Public advisories are published via GitHub Security Advisories after a fix ships.
