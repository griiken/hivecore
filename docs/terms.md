# Terms

## Licence

Hivecore is dual-licensed at the user's option:

- **MIT License** — see [../LICENSE-MIT](../LICENSE-MIT)
- **Apache License, Version 2.0** — see [../LICENSE-APACHE](../LICENSE-APACHE)

You may pick whichever licence suits your downstream use. The two licenses are independent grants; you do not need to comply with both simultaneously.

### Why dual?

- **MIT** maximizes adoption: no patent grant, minimal compliance burden.
- **Apache-2.0** provides an explicit patent grant and a clear contribution model.

This is the standard pattern in the Rust ecosystem and matches how most major Rust projects (rustc, tokio, serde, etc.) license themselves.

## Contributions

Contributions are accepted under the same dual MIT OR Apache-2.0 terms. Apache-2.0's explicit patent grant applies to all contributions.

Submitting a contribution implies:

1. You have the right to contribute the code (you wrote it, or you have permission from the owner).
2. You agree to license it under MIT OR Apache-2.0 at the user's option.
3. You grant Apache-2.0's patent terms.

No CLA (Contributor License Agreement) is required. The licence terms in `LICENSE-MIT` and `LICENSE-APACHE` are sufficient.

## Trademarks

"Hivecore" is the name of this project. It is not a registered trademark. If hivecore reaches significant adoption, a trademark filing may follow; rules will be documented here at that point.

You may use the name "hivecore" to refer to the project, in documentation, in compatibility statements ("works with hivecore"), and in derivative work names that are clearly distinct (e.g., "myorg-hivecore-fork").

You may not use the name "hivecore" in a way that suggests official endorsement of a fork, derivative, or commercial product without explicit written permission.

## Disclaimer

Hivecore is provided "AS IS" without warranty of any kind. See the licence files for full disclaimer text.

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
