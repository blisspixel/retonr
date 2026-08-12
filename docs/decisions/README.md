# Architecture decision records

Decision records capture choices that constrain public behavior, stored data,
security, dependencies, or delivery.

## Process

1. Copy [the template](0000-template.md).
2. Assign the next four-digit number.
3. Describe the context and decision drivers before selecting an option.
4. Record considered alternatives and their concrete tradeoffs.
5. Set the status to `proposed`.
6. Change the status to `accepted` only after the relevant roadmap gate passes.
7. Replace an accepted decision through a new record that links to the old one.

Decision records do not hide uncertainty. A time-bounded experiment records its
success and stop conditions.

## Records

| Record | Status | Decision |
| --- | --- | --- |
| [0001](0001-common-validation-cascade.md) | Proposed | One acceptance cascade for every strategy |
| [0002](0002-private-codename.md) | Superseded | Private codename and neutral internal namespaces |
| [0003](0003-artifact-qualification-activation.md) | Proposed | Separate artifact, qualification, and activation identity |
| [0004](0004-inference-port-and-ollama-transport.md) | Proposed | Backend-neutral inference and bounded Ollama transport |
| [0005](0005-grounded-strategy-authority.md) | Proposed | Grounded strategies propose but cannot accept or apply |
| [0006](0006-retonr-public-identity.md) | Accepted | Retonr public project identity and namespace migration |
