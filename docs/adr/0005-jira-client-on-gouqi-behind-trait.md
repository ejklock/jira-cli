---
type: ADR
title: "Build the Jira client on the gouqi crate, behind our own client trait"
description: There is no official Atlassian Rust SDK; build jira-cli's client layer on the maintained community crate gouqi (correct /rest/api/3/search/jql + Cloud Basic auth, reqwest/rustls), wrapped behind a thin client trait so token host-isolation, agent_json mapping, and a future swap stay ours.
status: Accepted
supersedes:
superseded_by:
tags: [client, sdk, dependency, jira, gouqi]
timestamp: 2026-06-29T00:00:00Z
---

# 0005. Build the Jira client on the gouqi crate, behind our own client trait

## Context

[ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md) said the `client`/`models`
domain layer would be rewritten for Jira and kept behind a clean trait seam.
[ADR 0002](/adr/0002-jira-cloud-only-basic-auth.md) fixed the target: Cloud REST v3,
Basic auth (`email:token`), JQL via `/rest/api/3/search/jql`. This ADR decides
**how** the client layer is built.

Findings (researched 2026-06-29):

- **No official Atlassian Rust SDK exists.** Atlassian's official surface is the
  REST API plus a published OpenAPI specification; their first-party SDK/framework
  tooling (Forge, Connect) is JS/TS. The "most official" Rust path is codegen from
  the OpenAPI spec.
- **`gouqi 0.20`** (a maintained fork of `softprops/goji`, MIT, last pushed
  2026-02) is the best-maintained community Rust Jira client. It matches our stack
  (`reqwest 0.12` + `rustls-tls`, optional `async`/`tokio`), and crucially it
  already implements the parts that are tedious and easy to get wrong:
  - `SearchApiVersion::V3` → `/rest/api/3/search/jql` with `nextPageToken`
    pagination, and deployment auto-detection (Cloud vs Server) by host.
  - `Credentials::Basic(email, api_token)` — exactly Jira Cloud API-token auth.
  - Modules covering v1 (`issues`, `search`, `users`) and Phase 2
    (`transitions`, `boards`, `sprints`, `attachments`, `projects`).

Force: **don't reinvent the Jira REST client.** The new `/search/jql` token
pagination and the v2→v3 deprecation handling are real work gouqi already did
correctly.

Tension: gouqi **owns its reqwest transport**, so our **token host-isolation**
non-negotiable stops being an explicit seam in our `http` module. In practice gouqi
is constructed per-instance with a single host base URL, so for read-only `get`/
`search` it only ever calls the configured Jira host — but we lose the direct
negative-test boundary the AC base had.

## Decision

Build the client layer **on gouqi, wrapped behind our own `client` trait**.

1. **Dependency.** Add `gouqi` (features `async`, `rustls-tls` via reqwest) to the
   crate. The fork inherits the AC `http` module; gouqi replaces the hand-rolled
   transport for Jira calls.
2. **Trait seam.** Define a thin `JiraClient` trait (e.g. `get_issue(key)`,
   `search(jql, opts)`, `myself()`) in our `client` module. The gouqi-backed
   implementation is one impl; the trait is what `controller`/`commands` depend on.
   This keeps [ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md)'s clean seam
   and lets us swap to a hand-rolled or OpenAPI-codegen client later without
   touching callers.
3. **Construction is host-pinned.** The gouqi `Jira` is built from the instance
   `base_url` and `Credentials::Basic(email, token)`. **Host isolation is enforced
   in the wrapper**: the wrapper is the single place a `Jira` is constructed, always
   from the resolved instance host — no caller passes an arbitrary URL. A unit test
   asserts the wrapper rejects/never issues a request to a host other than the
   instance host (the re-homed NFR-1 boundary).
4. **Mapping, not leaking.** gouqi's representation types (`rep`) are mapped to our
   own domain `models` at the wrapper boundary; gouqi types do **not** appear in
   `agent_json`, `render`, or the cache payload. The curated `agent_json` contract
   ([ADR 0004](/adr/0004-agent-json-output-contract.md)) is shaped from our domain
   types, so an upstream gouqi change cannot silently alter our contract.
5. **Search version pinned to V3.** The client is configured with
   `SearchApiVersion::V3` explicitly (not relying on auto-detect) since v1 is
   Cloud-only.

## Alternatives considered

- **Codegen from Atlassian's official OpenAPI spec** (progenitor / openapi-generator).
  Rejected for v1: the most "official" types, but a large generated client, heavy
  ADF types, and codegen/build complexity — and we would still hand-wire auth,
  cache, and host-isolation on top. Reconsider if gouqi proves limiting.
- **Hand-rolled reqwest client** (the original J0 plan). Rejected as the default:
  full control and an explicit host-isolation seam, but we would reimplement the
  `/search/jql` `nextPageToken` pagination and track API changes ourselves — exactly
  what gouqi already maintains. Kept as the fallback the trait seam enables.

## Consequences

**Positive:**

- v1's riskiest net-new layer (the Jira client) reuses a correct, maintained
  implementation of Cloud auth + v3 JQL search/pagination.
- Phase 2 (transitions, boards, sprints, attachments) has a ready surface.
- The trait seam preserves swappability and keeps our contract/domain decoupled
  from gouqi.

**Accepted trade-offs:**

- A dependency on a ~34★ community crate (bus factor). Mitigated by the trait seam
  (swap path) and by mapping gouqi types at the boundary (no deep coupling).
- Token host-isolation moves from an `http`-module seam to a wrapper invariant +
  unit test; the wrapper is the single `Jira` construction site to keep that true.
- A `gouqi` representation→domain mapping layer to write and test.

## Related

- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- ADR: [/adr/0002-jira-cloud-only-basic-auth.md](/adr/0002-jira-cloud-only-basic-auth.md)
- ADR: [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md)
- Issue: [/issues/0001-j0-skeleton-setup-get.md](/issues/0001-j0-skeleton-setup-get.md)
