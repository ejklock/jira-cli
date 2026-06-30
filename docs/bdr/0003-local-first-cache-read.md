---
type: BDR
title: "local-first cache read — offline get, --refresh re-fetch"
description: A cached issue is served from SQLite with no network; --refresh forces a re-fetch and overwrites the cached row.
status: Accepted
supersedes:
superseded_by:
tags: [cache, offline, local-first]
timestamp: 2026-06-29T00:00:00Z
---

# 0003. local-first cache read — offline get, --refresh re-fetch

## Context

Local-first is a constitution non-negotiable. After J0 writes issues to the cache,
slice J1 ([Issue 0002](/issues/0002-j1-local-first-cache.md)) makes reads serve
from it offline, under [PRD 0001](/prd/0001-jira-cloud-read-cli.md) R3 and
[ADR 0003](/adr/0003-issue-identity-and-cache-key.md).

## Behavior

```mermaid
flowchart TD
    A["jira get PROJ-123 [--refresh]"] --> C{cached (instance, key)?}
    C -->|hit and not --refresh| S[serve from cache, no network]
    C -->|miss| F[fetch → write → serve]
    C -->|hit and --refresh| F2{network up?}
    F2 -->|yes| F
    F2 -->|no| ERR[refresh error, exit 1, cache untouched]
```

## Textual Description

- **Cache hit, no refresh:** served entirely from SQLite; **no HTTP request** is
  made. Works with the network disabled.
- **Cache miss:** fetch, write `(instance, key)`, serve (the J0 path).
- **`--refresh`:** force the fetch arm even on a hit; on success overwrite the
  cached row; on a network failure, exit 1 and leave the existing cache row intact
  (no partial overwrite).
- **Single source of truth:** only `store` opens the SQLite file.

## Scenarios

**Scenario 1: offline cache hit** — with a cached `PROJ-123` and the network
disabled, `jira get PROJ-123` renders from cache; exit 0; zero HTTP requests.
**Scenario 2: --refresh re-fetches** — `jira get PROJ-123 --refresh` issues exactly
one fetch and overwrites the cached row.
**Scenario 3: refresh offline fails safe** — `--refresh` with the network down
exits 1 and leaves the prior cache row unchanged (a later offline read still works).
**Scenario 4: miss then hit** — first `get` (miss) writes the cache; an immediate
second `get` offline serves from it.

## Test Design

All integration-tested against **wiremock** + **temp SQLite**, asserting request
counts (the observable proof of "no network on a hit").

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Offline hit | integration | 1 | render, exit 0, wiremock received 0 requests | local-first read (NFR-2) |
| Refresh count | integration | 2 | exactly 1 fetch, row overwritten | refresh path |
| Refresh fail-safe | integration | 3 | exit 1, prior row intact | safe refresh on error |
| Miss→hit | integration | 4 | 1st writes, 2nd offline serves | cache lifecycle |

## Related

- PRD: [/prd/0001-jira-cloud-read-cli.md](/prd/0001-jira-cloud-read-cli.md)
- ADR: [/adr/0003-issue-identity-and-cache-key.md](/adr/0003-issue-identity-and-cache-key.md)
- Issue: [/issues/0002-j1-local-first-cache.md](/issues/0002-j1-local-first-cache.md)
