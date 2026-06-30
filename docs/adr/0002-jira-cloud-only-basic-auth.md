---
type: ADR
title: "Jira Cloud only for v1 (REST v3 + Basic auth email+API token)"
description: v1 targets Jira Cloud exclusively — REST v3, HTTP Basic auth with the account email and an API token, JQL search via /rest/api/3/search/jql — deferring Jira Server/Data Center (REST v2 / PAT) to Phase 2.
status: Accepted
supersedes:
superseded_by:
tags: [auth, api, jira, cloud, scope]
timestamp: 2026-06-29T00:00:00Z
---

# 0002. Jira Cloud only for v1 (REST v3 + Basic auth email+API token)

## Context

Jira has two deployment families with different APIs and auth: **Cloud** (REST v3,
HTTP Basic auth with `email:api_token`, JQL via `/rest/api/3/search/jql`) and
**Server / Data Center** (REST v2, PAT bearer). Supporting both means abstracting
two API dialects and two auth schemes, doubling the client surface and the mocked
tests.

Force: **v1 critical-path simplicity.** Cloud is the common case and has a single,
well-documented auth scheme. The existing `jira-ticket` skill already proves the
Cloud REST v3 + API-token path. Narrowing v1 to Cloud removes a whole abstraction
from the riskiest net-new layer (the API client).

## Decision

v1 supports **Jira Cloud only**.

1. **API:** Jira Cloud Platform REST v3 (`/rest/api/3/...`). Issue read via
   `/rest/api/3/issue/{key}`; search via `/rest/api/3/search/jql`.
2. **Auth:** HTTP Basic with the account email and an API token —
   `Authorization: Basic base64(email:token)`. The token is created by the user at
   id.atlassian.com and stored locally (plaintext for v1, per the constitution).
3. **Instance shape:** `base_url` is the Cloud site origin
   (`https://<site>.atlassian.net`); `setup add` resolves the authenticated user's
   `account_id` via `/rest/api/3/myself` and stores it.
4. **No Server/DC code path in v1.** The client is written so a Server/DC adapter
   can be added later behind the same `client` trait boundary, but no v2/PAT code
   ships in v1.

## Alternatives considered

- **Both, auto-detected by URL** (as the `jira-ticket` skill does). Rejected for
  v1 scope: doubles the client + auth surface on the riskiest layer for a case
  (on-prem) the owner does not need first. Deferred to Phase 2 behind the same
  trait seam.
- **Server/DC only.** Rejected: Cloud is the dominant deployment and the simpler,
  better-documented auth.

## Consequences

**Positive:**

- One API dialect, one auth scheme — the smallest possible net-new client.
- Reuses the proven Cloud path from the `jira-ticket` skill as a reference.

**Accepted trade-offs:**

- On-prem Jira Server/DC users are unserved until Phase 2.
- The `client` boundary must stay a clean seam so the v2 adapter is additive, not
  a rewrite (recorded as a fitness expectation, not yet enforced).

## Related

- Constitution: [/constitution.md](/constitution.md)
- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- BDR: [/bdr/0002-setup-instance-management.md](/bdr/0002-setup-instance-management.md)
