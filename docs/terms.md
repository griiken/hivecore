# Terms

## Licence

Hivecore is licensed under the **Apache License, Version 2.0** — see [../LICENSE](../LICENSE).

ADR-030 supersedes the earlier dual MIT-OR-Apache-2.0 choice (ADR-002) as of 2026-05-03. Snapshots of the tree at commits before that switch remain dual-licensed forever; the going-forward licence is Apache-2.0 only.

### Why Apache-2.0

- **Explicit patent grant** — protects users and contributors against patent-based reprisal in a way bare MIT cannot.
- **Clear contribution model** — Apache-2.0 §5 already governs the inbound contribution licence; no separate CLA needed.
- **One file, one rule** — drops the dual-licence indirection that confuses tooling, packagers, and downstream consumers.
- **Vendored prompts under `crates/hivecore-system-prompt/prompts/` are already Apache-2.0** (Codex / Zed) — single-license matches.

## Contributions

Contributions are accepted under the **Apache License, Version 2.0** terms. Submitting a contribution (PR, patch, issue with code) implies:

1. You have the right to contribute the code (you wrote it, or you have permission from the owner).
2. You licence your contribution under Apache-2.0 to the project and to downstream users.
3. You grant the patent terms of Apache-2.0 §3.

No CLA (Contributor License Agreement) is required. Apache-2.0 §5 ("Submission of Contributions") is sufficient.

## Trademarks

"Hivecore" is the name of this project. It is not a registered trademark. If hivecore reaches significant adoption, a trademark filing may follow; rules will be documented here at that point.

You may use the name "hivecore" to refer to the project, in documentation, in compatibility statements ("works with hivecore"), and in derivative work names that are clearly distinct (e.g., "myorg-hivecore-fork").

You may not use the name "hivecore" in a way that suggests official endorsement of a fork, derivative, or commercial product without explicit written permission.

## Disclaimer

Hivecore is provided "AS IS" without warranty of any kind. See the licence file for full disclaimer text.

Operating hivecore in production means running autonomous agents that modify code and external systems. Operators are responsible for:

- Configuring sandboxing, secrets, and budgets appropriately.
- Reviewing agent-generated PRs before merge to production branches.
- Complying with their own organization's compliance requirements.
- Securing their LLM API keys.

The authors and contributors of hivecore are not liable for damages arising from misuse, misconfiguration, or production incidents.

## Privacy

Hivecore is self-hosted. The authors of hivecore do not collect any data from your deployments.

If you opt in to a future managed hivecore cloud service (when one exists), a separate privacy policy will apply.

## Updates

Material changes to this terms file will be announced in [CHANGELOG.md](../CHANGELOG.md) and reflected in a new git tag.
