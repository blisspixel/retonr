# Architecture decision records

Decision records capture choices that constrain public behavior, stored data,
security, dependencies, or delivery.

## Process

1. Copy [the template](0000-template.md).
2. Assign the next four-digit number.
3. Describe the context and decision drivers before selecting an option.
4. Record considered alternatives and their concrete tradeoffs.
5. Set the status to `proposed`.
6. Keep 0.x working decisions `proposed` while evidence can still materially change
   them. Accept compatibility decisions during 0.9 qualification or earlier only
   when an irreversible external action requires a frozen choice.
7. Replace an accepted decision through a new record that links to the old one.

Decision records do not hide uncertainty or block reversible exploration merely
because a preview choice is still proposed. A time-bounded experiment records its
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
| [0007](0007-development-host-identity.md) | Proposed | Development host identity and hardware-probe privacy fields |
| [0008](0008-attached-process-witness.md) | Proposed | Bounded native witness for an attached runtime process |
| [0009](0009-retained-connection-attribution.md) | Proposed | One retained Ollama transport with repeated native connection attribution |
