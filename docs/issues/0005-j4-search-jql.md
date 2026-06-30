---
type: Issue
title: "J4 — search: arbitrary JQL"
description: Run a user-supplied JQL query through the shared listing engine and render the matches.
status: open
tracker:
tags: [search, jql]
timestamp: 2026-06-29T00:00:00Z
---

# J4 — search: arbitrary JQL

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R6 →
[BDR 0005](/bdr/0005-mine-and-search-jql.md) → in force
[ADR 0004](/adr/0004-agent-json-output-contract.md).

## Context manifest

- **Touch:** `commands::search` (new handler) over the J3 listing engine —
  `client::search_jql`, the table renderer, and the `agent_json` list schema are
  already built; this slice sends the user's JQL verbatim and surfaces a 400.
- **Error:** a Jira 400 (invalid JQL) maps to `invalid JQL: {server message}`,
  exit 1.

## Vertical Demo

- **Given** the configured instance,
  **When** I run `jira search "project = PROJ ORDER BY updated DESC"` and
  `jira search "..." --json`,
  **Then** the matching issues print as the table / curated list object.
- **Unhappy path:** **Given** a malformed query,
  **When** I run `jira search "project = "`,
  **Then** it prints `invalid JQL: ...` (the server message) and exits 1.

## Acceptance

| AC | Condition | Instrument |
|---|---|---|
| AC1 | User JQL sent verbatim, matches listed (BDR 0005 Scn 3) | wiremock integration test |
| AC2 | `--json` list shape (Scn 3) | integration test |
| AC3 | Invalid JQL 400 → `invalid JQL: ...`, exit 1 (Scn 4) | integration test |
| AC4 | Empty result → `No issues.`, exit 0 (Scn 5) | integration test |
| AC5 (constraint) | why-only comments / no banners | inspection + comment_policy |
| AC6 (constraint) | Cyclomatic ≤ 10 / cognitive within gate | quality-gate complexity |
| AC7 (constraint) | Mutants on changed lines killed | quality-gate mutation (reviewer backstop) |
| AC8 (constraint) | Honors the Design principles (deep modules · thin commands · FCIS · one seam) — see [architecture](/architecture.md) | inspection (Reviewer) |

## Out of scope

- JQL autocomplete/validation client-side; saved queries (Phase 2).
- Full pagination (PRD open question).

## blocked_by

[0004](/issues/0004-j3-mine-list.md)
