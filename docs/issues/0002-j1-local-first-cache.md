---
type: Issue
title: "J1 — local-first cache read: offline get + --refresh"
description: Serve a cached issue from SQLite with no network; --refresh forces a safe re-fetch.
status: open
tracker:
tags: [cache, offline, refresh]
timestamp: 2026-06-29T00:00:00Z
---

# J1 — local-first cache read: offline get + --refresh

## Objective link

[Constitution](/constitution.md) (local-first non-negotiable) →
[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R3 →
[BDR 0003](/bdr/0003-local-first-cache-read.md) → in force
[ADR 0003](/adr/0003-issue-identity-and-cache-key.md).

## Context manifest

- **Touch:** `controller` (cache-first branch + `--refresh` arm), `store` cache
  read/write, `commands::get` flag wiring. No new modules.
- **Pattern:** the AC controller's cache-hit / fetch / single-flight refresh shape
  is the reference; adapt the key to `(instance, issue_key)`.
- **Test harness:** wiremock request-count assertions + temp SQLite (existing).

## Vertical Demo

- **Given** I have run `jira get PROJ-123` once (cache warm),
  **When** I disable the network and run `jira get PROJ-123` again,
  **Then** it renders the issue from cache and exits 0.
- **Given** the warm cache and network up,
  **When** I run `jira get PROJ-123 --refresh`,
  **Then** it re-fetches and the updated fields show.
- **Unhappy path:** **Given** a warm cache and the network **down**,
  **When** I run `jira get PROJ-123 --refresh`,
  **Then** it exits 1 and a subsequent offline `jira get PROJ-123` still renders
  the prior cached issue (the row was not clobbered).

## Acceptance

| AC | Condition | Instrument |
|---|---|---|
| AC1 | Offline cache hit makes 0 HTTP requests (BDR 0003 Scn 1, NFR-2) | wiremock request-count integration test |
| AC2 | `--refresh` issues exactly 1 fetch + overwrites row (Scn 2) | integration test |
| AC3 | `--refresh` offline → exit 1, prior row intact (Scn 3) | integration test |
| AC4 | miss→hit lifecycle (Scn 4) | integration test |
| AC5 (constraint) | No superfluous comments; why-only | inspection + comment_policy |
| AC6 (constraint) | Cyclomatic ≤ 10 / cognitive within gate | quality-gate complexity |
| AC7 (constraint) | Mutants on changed lines killed | quality-gate mutation (reviewer backstop) |
| AC8 (constraint) | Honors the Design principles (deep modules · thin commands · FCIS · one seam) — see [architecture](/architecture.md) | inspection (Reviewer) |

## Out of scope

- Per-issue caching of `mine`/`search` list results (PRD open question).
- Cache TTL/SWR for lists (Phase 2 with the TUI).

## blocked_by

[0001](/issues/0001-j0-skeleton-setup-get.md)
