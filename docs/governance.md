# Governance

How decisions get made in hivecore.

## Current state

Pre-v1.0. **Benevolent Dictator (BDFL) model** with a transparent ADR process.

- BDFL: griflet (project founder, dipendra497@gmail.com).
- Decisions documented in [DECISIONS.md](./DECISIONS.md).
- Community input solicited via `rfc:`-labelled issues with one-week comment periods.

## Roles

### Maintainer
Has commit and merge rights. Reviews and merges PRs. Currently: founder only. Adding maintainers requires founder approval and a track record (≥5 merged non-trivial PRs).

### Contributor
Anyone who opens issues, PRs, or participates in discussions. Welcome.

### Reviewer
A contributor explicitly tagged for domain review on specific PRs. Not a permanent role.

## Decision types

### Trivial
Typo, comment fix, small refactor with no behavior change. Single maintainer approval suffices.

### Non-trivial
New feature, behavior change, dependency add. Requires `rfc:` issue with one-week public comment period. Maintainer merges after consensus or BDFL ruling.

### Architecturally significant
Schema changes, new core abstractions, breaking changes, license changes. Requires:
- `rfc:` issue with two-week comment period
- ADR added to [DECISIONS.md](./DECISIONS.md)
- Maintainer + BDFL approval

### Security-sensitive
See [SECURITY.md](../SECURITY.md). Coordinated disclosure; BDFL approves disclosure timeline.

## Forking and disagreement

Hivecore is Apache-2.0 (ADR-030; superseded ADR-002). Disagree with a direction? Fork freely. We will publicize legitimate forks in [comparison.md](./comparison.md).

## Future evolution

Once hivecore reaches v1.0 + meaningful adoption (>50 contributors, >5 production deployments), governance migrates to one of:

- **Foundation governance** — Linux Foundation, Apache Software Foundation, or similar. Donor org decides.
- **Steering committee** — 3-5 maintainers from independent organizations vote on architecturally significant changes.
- **Subsystem maintainers** — KG, orchestrator, frontend, etc. each have a lead maintainer.

The transition itself will be an architecturally significant decision, going through the full process above.
