---
type: Issue
title: "J2 — current: issue from the git branch"
description: Parse a Jira issue key from the current git branch and read that issue through the get path.
status: open
tracker:
tags: [current, git]
timestamp: 2026-06-29T00:00:00Z
---

# J2 — current: issue from the git branch

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R4 →
[BDR 0004](/bdr/0004-current-from-git-branch.md) → reuses
[BDR 0001](/bdr/0001-get-issue-by-key.md).

## Context manifest

- **Touch:** `commands::current` (new handler), a pure `extract_issue_key(branch)`
  helper (regex `[A-Z][A-Z0-9]+-\d+`, first match), branch read via git, and
  bare-invocation normalization in `cli` (mirror the AC `current` shortcut).
- **Reuse:** the J0 `get` controller path verbatim once the key is extracted.

## Vertical Demo

- **Given** I am on branch `feature/PROJ-123-add-login`,
  **When** I run `jira current` (and `jira current --json`),
  **Then** it renders `PROJ-123` (human, then curated object).
- **Unhappy path:** **Given** I am on branch `main`,
  **When** I run `jira current`,
  **Then** it prints a "no issue key in branch" message and exits 2, making no
  network call.

## Acceptance

| AC | Condition | Instrument |
|---|---|---|
| AC1 | Key extraction across branch shapes (BDR 0004 Scn 1,2,5; multi-token) | pure unit tests |
| AC2 | `current` end-to-end renders the branch issue (Scn 1) | integration test (stubbed branch) |
| AC3 | No key / not a repo → exit 2, 0 requests (Scn 3) | integration test |
| AC4 | `--json` passthrough (Scn 4) | integration test |
| AC5 (constraint) | why-only comments / no banners | inspection + comment_policy |
| AC6 (constraint) | Cyclomatic ≤ 10 / cognitive within gate | quality-gate complexity |
| AC7 (constraint) | Mutants on changed lines killed | quality-gate mutation (reviewer backstop) |
| AC8 (constraint) | Honors the Design principles (deep modules · thin commands · FCIS · one seam) — see [architecture](/architecture.md) | inspection (Reviewer) |

## Out of scope

- Non-Jira branch conventions; configuring a custom key pattern (Phase 2).

## blocked_by

[0001](/issues/0001-j0-skeleton-setup-get.md)
