# ADR 0004: Inference port and Ollama transport boundary

- Status: proposed
- Decision owners: project maintainers
- Decision gate: roadmap milestone 0.2 entry closure
- Last reviewed: 2026-08-12

## Context

The CLI, desktop application, API, MCP server, evaluation harness, and later speech
features must not depend on one runtime's wire types. At the same time, a local HTTP
runtime is not an in-process trusted component. It can return malformed or oversized
data, redirect requests, report mutable tags, change during a call, or retain
undocumented defaults.

The first adapter targets Ollama's native API. The design must stay narrow enough to
qualify and replace without moving transport concerns into the deterministic engine.

## Decision drivers

- Backends remain replaceable and testable without a real model.
- Complete prompt, source, candidate, body, time, and cancellation bounds are
  explicit.
- Local-only mode cannot be bypassed by DNS, proxies, or redirects.
- No qualified run depends on a mutable tag or undocumented generation default.
- Errors and traces do not expose source, prompt, candidate, or protected values.

## Options considered

### Call Ollama directly from each interface

This minimizes initial code but duplicates security, identity, retry, and response
handling. It also prevents shared conformance tests.

### Depend on an Ollama client library throughout the application

A client library reduces wire code but exposes runtime-specific types and may choose
proxy, redirect, retry, streaming, or default behavior outside product policy.

### Backend-neutral port with a narrow native adapter

Define owned versioned requests, boxed object-safe futures, borrowed cancellation and
deadline state, stable error categories, exact identity, and bounded responses. Keep
the Ollama HTTP implementation in a separate adapter crate.

## Decision

Select the backend-neutral port and narrow native adapter.

The inference request carries exact artifact identity, observed source bytes,
qualified source capacity, complete serialized-input capacity, context setting,
output schema and digest, candidate count and size, sampling values, seed, and
reasoning policy. The response carries exact runtime and artifact identity, ordered
bounded candidates, and optional usage observations.

The initial Ollama adapter accepts only an HTTP URL with an IP-literal loopback host.
It disables system proxies, redirects, implicit retries, and referrer behavior. It
uses HTTP/1, explicit connect, read, request, and operation deadlines, checks content
type and body length before and during accumulation, and returns redacted stable
errors.

Discovery uses native version, tag, and model-detail endpoints. Runtime references
are configured only through an explicit binding to an expected artifact digest. The
adapter checks that binding before and after generation, requires completion
capability, sends non-streaming structured output, disables reasoning, and sets every
qualified generation option explicitly. It never starts, installs, pulls, updates,
or removes the runtime or a model.

## Consequences

### Positive

- Interfaces and evaluation share one inference contract and fake.
- Transport policy is reviewable in one small crate.
- Digest drift and runtime changes discard generated results.
- Later backends can reuse the same conformance cases.

### Negative

- The adapter supports a deliberately small Ollama API subset.
- Runtime-native digest meaning still requires artifact qualification evidence.
- Non-streaming output increases peak response latency and buffering, though it
  avoids exposing unvalidated partial text.

### Follow-up

- Add delayed, disconnected, partial, redirected, chunked-oversize, and deadline
  conformance cases.
- Record endpoint configuration precedence before exposing user configuration.
- Add a socket-denial qualification test for local-only operation.
- Qualify exact Ollama and artifact versions separately from fake-server tests.

## Validation

The decision passes when fake-server conformance proves loopback rejection,
proxy and redirect denial, body bounds, cancellation, deadline behavior, malformed
response handling, exact request parameters, pre-call drift rejection, post-call
drift discard, and absence of content in errors and traces.

Revisit if the selected runtime removes required identity or structured-output
capabilities, or if a product-managed in-process runtime provides a smaller and more
verifiable boundary.

## References

- [Ollama version API](https://docs.ollama.com/api-reference/get-version)
- [Ollama model list API](https://docs.ollama.com/api/tags)
- [Ollama model details API](https://docs.ollama.com/api-reference/show-model-details)
- [Ollama generation API](https://docs.ollama.com/api/generate)
- [reqwest client controls](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)
- [0.2 execution plan](../planning/0.2-grounded-cli.md)
