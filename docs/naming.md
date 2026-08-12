# Naming status

## Decision status

`Retonr` is the selected public project identity. It is pronounced "ree-tone-er"
and describes returning a machine-shaped or rough draft to the user's own tone
while preserving the underlying intent and declared constraints.

The spelling is consistent with the maintainer's existing compact project naming
pattern. The executable and public-facing package use `retonr`. Internal library
crates retain namespace-neutral `rewrite-*` names so architecture does not depend
on presentation branding.

The accepted decision and migration scope are recorded in
[ADR 0006](decisions/0006-retonr-public-identity.md).

## Positioning rule

The name does not imply that Retonr can erase upstream provider logs, prove human
authorship, defeat every classifier, or remove every form of provenance. Product
copy must describe source-form and metadata handling as bounded, supported, and
reported behavior.

The stable description is:

> Retonr locally reconstructs machine-generated and rough drafts in the user's own
> writing style while preserving intent and handling supported text and document
> artifacts.

## Namespace evidence

The publication decision included exact-name checks on August 12, 2026 for:

- GitHub account and repository names
- crates.io
- npm
- PyPI
- `retonr.com`
- `retonr.app`
- `retonr.dev`
- General web and confusingly similar software uses

The checked exact namespaces appeared available at the time of the decision, and no
confusingly similar writing or local-AI product was identified in the preliminary
screen. Availability can change. These checks are product research, not a legal
opinion or trademark clearance.

## Publication and release rule

The public source repository is authorized under the `Retonr` name. Publishing a
package, installer, signed desktop application, hosted service, or 1.0 release still
requires:

1. Formal legal review appropriate to the intended distribution and jurisdictions.
2. Reconfirmation of repository, package, application, domain, and executable
   namespaces immediately before reservation or publication.
3. Cross-platform rename, packaging, upgrade, and screenshot conformance evidence.
4. Consistent use of `Retonr` in product copy and `retonr` in technical namespaces.
5. A compatibility decision if any previously distributed identifier must be
   retained or redirected.

Implementation does not wait for package publication. Public APIs, stored schemas,
and configuration namespaces remain provisional until their roadmap gates pass.
