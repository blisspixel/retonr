# Provenance, marking, and derivative handling

## Purpose

Retonr treats provenance as evidence attached to, bound to, or held outside an
artifact. It does not treat provenance as an authorship verdict. This contract
governs supported signatures, Content Credentials, metadata, invisible controls,
statistical watermark declarations, provider records, and derivative output.

The complete evidence review and current legal context are recorded in
[Text provenance, marking, and editorial control](research/2026-08-12-provenance-policy.md).
The technical watermark boundary is recorded in
[Text watermark science and Retonr implications](research/2026-08-12-text-watermark-science.md).

## Invariants

- Inspect supported carriers before normalization, model execution, or output
  mutation.
- Preserve unknown metadata, package parts, and format controls by default.
- Never represent an invalidated signature or binding as valid on a derivative.
- Keep the source artifact unchanged and write a separate derivative by default.
- Require an explicit derivative decision when a recognized binding will change.
- Record what was preserved, superseded, invalidated, unsupported, unresolved,
  removed for a documented purpose, or not checked.
- Never add a shared signing key, fabricate a signer, or invent a provenance claim.
- Never expose a watermark-removal, detector-evasion, human-authorship, or
  provenance-clean success state.
- Never query an external manifest, provider, trust service, revocation service, or
  detector without explicit network authority.

These controls protect document integrity and accurate claims. They do not decide
whether the user's expression is permissible or whether a disclosure applies.

## Mechanism classes

| Mechanism | Location | Retonr treatment |
| --- | --- | --- |
| Statistical linguistic watermark | Token choices, semantic representations, or trained behavior | Record a known runtime declaration; never use its detector in live ranking |
| C2PA Content Credential | Embedded, referenced, repository-linked, or text wrapper manifest | Validate supported states before editing; use an explicit derivative path |
| Native PDF or OOXML signature | Signed byte ranges or package parts | Inspect permissions and coverage; block silent invalidation |
| Mutable document metadata | XMP, core properties, custom fields, history, comments | Preserve by default; report exact qualified changes |
| Unicode or structural mark | Variation selectors, controls, whitespace, punctuation, markup | Classify before sanitation; preserve language and accessibility behavior |
| External evidence | Provider logs, fingerprints, manifests, account records, copies | State that local rewriting cannot inspect or delete it |
| Generic source classifier | A probabilistic model with no embedded evidence | Research diagnostic only; not a provenance fact or authorship verdict |

C2PA 2.4 defines carriers for unstructured text, structured text, PDF, and OOXML.
The unstructured-text carrier can use a byte-order mark followed by invisible
variation selectors. An invisible sequence is therefore not automatically junk or
safe to remove.

## Inspection states

The scanner represents independent facts rather than one `valid` badge:

- Carrier presence, absence, parse failure, unsupported state, and partial support
- Content-binding match or mismatch
- Cryptographic signature validity
- Signer trust under an exact trust policy and list
- Revocation and time-stamp results
- External reference resolved, unresolved, inaccessible, or not requested
- Unknown assertions, parts, metadata, and Unicode controls

A binding match means defined bytes or parts match a signed claim. It does not mean
`human`, `original`, `true`, `owned`, `authorized`, or `compliant`.

## Operation flow

1. Snapshot source bytes, media type, size, and digest.
2. Parse the container under explicit resource bounds without model execution.
3. Inventory supported and unknown metadata, signatures, credentials, controls, and
   external references.
4. Validate locally supported states using exact validator, specification, trust,
   and Unicode versions.
5. Determine whether the proposed edit preserves, supersedes, invalidates, or cannot
   interpret each carrier.
6. Present a preflight summary and include the decision in the immutable operation
   plan.
7. Keep the source unchanged. Stage and verify any approved derivative.
8. Emit a local, content-minimized report derived from source and output bytes.

Malformed, partially interpreted, externally unresolved, and unsupported bindings
block by default when safe derivative handling cannot be established.

## Derivative contract

A proposed content edit that invalidates a recognized valid signature or hard
binding does not silently continue. The user must explicitly select a qualified
derivative workflow after seeing the affected states.

The derivative workflow:

- Retains the original artifact and its pre-edit validation record
- Does not copy an invalidated signature as though it covers the derivative
- Uses an ingredient or history relationship when the format and configured signer
  can represent that relationship accurately
- Signs only with an explicitly configured key under a separately qualified profile
- Produces an unsigned derivative only after explicit confirmation when accurate
  derivative credentials cannot be emitted
- States that the source credential does not authenticate the derivative unless a
  new valid credential binds them

Retonr does not edit an existing signed claim. It does not infer a model identity,
human-oversight level, source type, or publication status for a new claim.

## Sanitation contract

Sanitation is separate from rewriting and provenance. It may remove an exact item
only for a documented security, privacy, interoperability, accessibility, or repair
purpose when all of these conditions hold:

1. Inspection did not classify the target as a recognized credential, mandatory
   transparency carrier, valid signature, or preservation-critical format state.
2. The user selects the exact carrier class or ranges after preview.
3. The adapter verifies visible text, logical order, language shaping,
   accessibility, structure, and protected literals.
4. The source remains unchanged and the output is a derivative.
5. The report records every removal and its stated purpose.

Suspicious does not mean malicious, watermarked, or safe to delete. Legitimate
bidirectional controls, joiners, variation selectors, soft hyphens, byte-order
marks, and accessibility data must not be stripped by a generic rule.

## Statistical watermark boundary

Linguistic reconstruction changes token and sentence choices and may incidentally
change a scheme-specific detector result. Retonr does not promise that a rewrite
preserves or removes a statistical watermark.

The live engine never:

- Queries a watermark detector while proposing, retrying, ranking, or accepting
  candidates
- Searches for a below-threshold output
- Recovers provider keys or estimates a hidden token partition
- Uses watermark-specific substitutions or translation loops
- Reports `watermark removed`, `undetectable`, `human-written`, or `clean`

A separately authorized research harness may exercise public schemes with synthetic
keys and locked data. Its results remain scheme-specific diagnostics and cannot
alter a production rewrite.

## Report contract

The private local report includes, as applicable:

- Source and output digests, media types, adapters, scanners, and validators
- Exact specification, trust-policy, trust-list, and Unicode identities
- Carrier types, locations, independent validation states, and redaction status
- Source and output metadata differences
- Inserted or removed Unicode scalars and byte ranges in escaped form
- The user's derivative or sanitation decision
- Whether the source remained unchanged
- Derivative credential and configured signer status
- External checks requested, completed, skipped, failed, or inaccessible
- Known runtime watermark declarations and exact limitations

Raw metadata can contain personal and operational data. Human, agent, and default
diagnostic output shows minimized states and local references. Raw values require an
explicit detailed report and user-selected destination.

## Release gates

- Pin the supported carrier, standard, validator, trust, Unicode, and format matrix.
- Add positive, negative, malformed, multiple-carrier, external-reference, unknown,
  binding-failure, signature-failure, and Unicode-security fixtures.
- Preserve unchanged paths byte-for-byte.
- Add a minimized regression fixture for every corrected provenance or structure
  defect.
- Use a maintained validator or an isolated adapter with conformance evidence. Do
  not present a partial parser as complete validation.
- Test deterministic offline behavior and separately authorized online resolution.
- Keep test signing keys visibly non-production and outside shipping artifacts.
- Run a documented project-side legal review for each distribution and service mode
  affected by current transparency law.
- Revalidate when a carrier, standard, validator, trust list, Unicode version,
  adapter, model runtime, output policy, distribution role, or relevant law changes.
