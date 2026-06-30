---
type: Issue
title: "J3 — mine/list: open issues assigned to me (JQL)"
description: List open issues assigned to the authenticated user via JQL, rendered as a table or the curated --json list.
status: open
tracker:
tags: [mine, list, jql]
timestamp: 2026-06-29T00:00:00Z
---

# J3 — mine/list: open issues assigned to me (JQL)

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R5 →
[BDR 0005](/bdr/0005-mine-and-search-jql.md) → in force
[ADR 0004](/adr/0004-agent-json-output-contract.md).

## Context manifest

- **Touch:** `client` (`POST /rest/api/3/search/jql`), `models` (`SearchResult`,
  `IssueRow`), `commands::mine` (alias `list`), `render` (issue table),
  `agent_json` (list schema `{count, jql, issues}`). This is the shared JQL listing
  engine J4 also uses.
- **JQL:** fixed `assignee = currentUser() AND statusCategory != Done ORDER BY
  updated DESC`; `--limit` caps page size.

## Vertical Demo

- **Given** the configured instance and issues assigned to me,
  **When** I run `jira mine` and `jira mine --json`,
  **Then** the first prints the `KEY · TYPE · STATUS · ASSIGNEE · SUMMARY` table
  and the second the one-line `{count, jql, issues}` object.
- **Unhappy path:** **Given** a query that matches nothing (e.g. no issues
  assigned),
  **When** I run `jira mine`,
  **Then** it prints `No issues.` and exits 0 (not an error).

## Acceptance

| AC | Condition | Instrument |
|---|---|---|
| AC1 | currentUser JQL built exactly (BDR 0005 Scn 1) | pure unit test |
| AC2 | Table render contract (Scn 1) | unit test |
| AC3 | `--json` list shape locked (Scn 2) | `agent_json` unit test |
| AC4 | mine end-to-end calls search with the JQL (Scn 1) | wiremock integration test |
| AC5 | Empty result → `No issues.`, exit 0 (Scn 5) | integration test |
| AC6 | `--limit` honored (Scn 6) | integration test |
| AC7 | Token host isolation (NFR-1) | negative unit test |
| AC8 (constraint) | why-only comments / no banners | inspection + comment_policy |
| AC9 (constraint) | Cyclomatic ≤ 10 / cognitive within gate | quality-gate complexity |
| AC10 (constraint) | Mutants on changed lines killed | quality-gate mutation (reviewer backstop) |
| AC11 (constraint) | Honors the Design principles (deep modules · thin commands · FCIS · one seam) — see [architecture](/architecture.md) | inspection (Reviewer) |

## Out of scope

- Full pagination beyond the first page (PRD open question).
- Per-issue caching of list results.

## blocked_by

[0001](/issues/0001-j0-skeleton-setup-get.md)
