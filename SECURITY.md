# Security policy

## Supported versions

Retonr is pre-release software. Security fixes are applied to the current `main`
branch. No tagged version is currently supported for production use.

## Reporting a vulnerability

Use GitHub private vulnerability reporting for this repository. Include:

- The affected commit, component, and platform
- Reproduction steps or a minimal proof of concept
- The expected and observed security boundary
- Potential impact on document confidentiality, integrity, availability, model
  artifacts, local services, or generated output
- Any suggested mitigation, if available

Do not open a public issue, discussion, or pull request for an undisclosed
vulnerability. Do not include real user documents, credentials, private model
artifacts, or unrelated personal data in a report.

The project aims to acknowledge a private report within seven calendar days.
Validation, remediation, disclosure timing, and credit will be coordinated in the
private report. A report may be closed when it cannot be reproduced, falls outside
the documented security boundaries, or does not create a security impact.

## Security boundaries

The current threat model and trust boundaries are documented in
[the security design](docs/security.md). Important non-guarantees include:

- Retonr cannot erase logs held by an upstream provider.
- Rewriting is not proof of human authorship or anonymity.
- Unkeyed content digests are identifiers, not anonymization.
- Model output is untrusted until it passes the common validation cascade.
- Loopback services, local files, model artifacts, and imported documents remain
  security-sensitive inputs.

Please report a boundary failure even when Retonr abstains or returns the original
document, because diagnostics, timing, storage, or partial processing may still
create a security issue.
