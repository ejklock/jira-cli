---
type: PRD
title: "Jira Cloud read/browse CLI (v1)"
description: A local-first Rust CLI to read and list Jira Cloud issues across instances — setup, get, current, mine/list, search (JQL) — with dual human + agent_json output.
status: Accepted
supersedes:
superseded_by:
tags: [cli, jira, read, v1]
timestamp: 2026-06-29T00:00:00Z
---

# 0001. Jira Cloud read/browse CLI (v1)

## Problem

Developers who live in the terminal lose context every time they have to open a
browser to read the Jira issue behind a branch, see what is assigned to them, or
run a quick query. There is no fast, scriptable, local-first way to read Jira
Cloud issues from the shell. A proven base exists — `active-collab-cli` solves the
same shape of problem for ActiveCollab — so the cost to deliver this for Jira is a
fork plus an API-layer swap, not a new product.

## Goals

1. Read a single Jira Cloud issue from the terminal by key, URL, or the current
   git branch, in under a second when cached.
2. List the issues assigned to me and run arbitrary JQL without leaving the shell.
3. Be scriptable: every read command emits a stable, low-token `agent_json`
   contract for agents and scripts.
4. Work offline against previously-fetched issues (local-first).
5. Support multiple Jira Cloud instances.

## Non-goals (v1)

- Writing to Jira (create/edit/comment/transition/worklog) — later slice.
- The interactive `browse` TUI — re-enabled from the fork in Phase 2.
- Jira Server / Data Center (REST v2 / PAT) — Cloud-only in v1.
- Secret-at-rest encryption; native pre-built binaries.

## Requirements

Each requirement is delivered by the BDR(s) and issue(s) it links.

- **R1 — Instance setup.** `setup add/list/remove/test` manages Jira Cloud
  instances (base_url, email, API token); `add` resolves and stores the
  `account_id`. Verified by [BDR 0002](/bdr/0002-setup-instance-management.md).
- **R2 — Get an issue.** `get <KEY|URL>` fetches and renders one issue
  (human + `--json`), writing it to the cache. Verified by
  [BDR 0001](/bdr/0001-get-issue-by-key.md).
- **R3 — Local-first read.** A cached `get` is served from SQLite with no network;
  `--refresh` forces a re-fetch. Verified by
  [BDR 0003](/bdr/0003-local-first-cache-read.md).
- **R4 — Current from branch.** `current` derives the issue key from the git
  branch and reads that issue. Verified by
  [BDR 0004](/bdr/0004-current-from-git-branch.md).
- **R5 — Mine / list.** `mine` (alias `list`) lists open issues assigned to the
  authenticated user via JQL `assignee = currentUser()`. Verified by
  [BDR 0005](/bdr/0005-mine-and-search-jql.md).
- **R6 — Search.** `search "<JQL>"` runs an arbitrary JQL query and lists the
  matching issues. Verified by [BDR 0005](/bdr/0005-mine-and-search-jql.md).
- **R7 — Dual output.** Every read command supports a human rendering and a
  curated minified `--json` contract derived from the same helpers. Verified by
  [ADR 0004](/adr/0004-agent-json-output-contract.md) and each command BDR.
- **R8 — i18n.** Human output is available in English and Brazilian Portuguese.

## Acceptance

The v1 critical path is acceptable when, against a real Jira Cloud instance:

1. `jira setup add --name work --url https://acme.atlassian.net --email me@acme.com`
   (token prompted) stores the instance with a resolved `account_id` and prints a
   connectivity line.
2. `jira get PROJ-123` prints the issue (summary, status, assignee, description)
   and `jira get PROJ-123 --json` prints the curated one-line object.
3. With the network disabled, `jira get PROJ-123` still succeeds from cache;
   `--refresh` re-fetches when online.
4. On branch `feature/PROJ-123-...`, bare `jira current` resolves and prints
   `PROJ-123`.
5. `jira mine` lists my open issues; `jira search "project = PROJ ORDER BY updated DESC"`
   lists matches. Both support `--json`.

## Quality-attribute scenarios (NFR, instrument-bound)

| ID | Scenario | Measure | Instrument |
|---|---|---|---|
| NFR-1 Token isolation | An issue fetch and any follow-up request | No `Authorization` header is attached to a host other than the instance's `base_url` host | Negative unit test on the http boundary |
| NFR-2 Offline read | `get` for a cached issue, network down | Exit 0, issue rendered from cache | wiremock-down integration test |
| NFR-3 Contract stability | Any read command `--json` | Field set matches the locked schema; rename/drop fails | `agent_json` unit tests |
| NFR-4 Single binary | Release artifact on a clean host | Runs with no runtime installed | Docker release build smoke |

## Open questions

- **JQL pagination.** Jira Cloud's `/rest/api/3/search/jql` is token-paginated.
  v1 lists the first page with a configurable `--limit`; full pagination is
  deferred to a named later slice unless a result set demands it.
- **Issue URL forms.** `get` must accept both the `PROJ-123` key and a browser URL
  (`.../browse/PROJ-123`). Other URL shapes (board/backlog deep links) are out of
  scope for v1.

## References

- Constitution: [/constitution.md](/constitution.md)
- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- ADR: [/adr/0002-jira-cloud-only-basic-auth.md](/adr/0002-jira-cloud-only-basic-auth.md)
