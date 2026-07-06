---
type: ADR
title: First-paint-from-cache SWR on browse entry (task-list snapshot)
description: Wire the inherited TaskListCache so entering browse paints the last-known mine list instantly, then always revalidates in the background and swaps — with pure single-flight and late-result guards in the TEA update. Cold entry keeps the existing pre-TUI blocking fetch. Port of fork-base ADR 0017 / BDR 0011, adapted.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, cache, swr, entry, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0016. First-paint-from-cache SWR on browse entry

## Context

Entering `jira browse` blocks the terminal on the network: `fetch_and_run`
(`src/tui/shell.rs`) runs `client.search(MINE_JQL, …)` **to completion before
the TUI opens**. The fork base solved this with a task-list snapshot cache and
entry-only stale-while-revalidate (fork ADR 0017, realized by fork BDR 0011 and
guarded by fork BDR 0005's single-flight rule). [PRD 0003](/prd/0003-active-collab-parity.md)
R-E3 ports that end state.

The store side is **already here**: the fork's `TaskListCache`
(`src/store/cache.rs`, table `task_list_cache` keyed by
`(scope, instances_key)` with `fetched_at` age validation) was carried over in
the fork scaffold and is fully unit-tested (TTL boundary + per-key isolation in
`tests/unit/store/cache.rs`) — it was just never wired to the TUI.

## Decision

Make browse **entry** stale-while-revalidate, reusing `TaskListCache` as-is.

1. **Warm entry seeds from the snapshot; the loop always revalidates.**
   `fetch_and_run` reads `TaskListCache("mine", instances_key, MAX_AGE)`
   (constructed from the same connection via `TaskCache::conn()`; `MAX_AGE` =
   7 days — a generous guard against absurdly old snapshots, since a
   revalidation always follows). Hit → deserialize `Vec<IssueRow>`, open the
   TUI immediately with `revalidating: true` and `next_page_token: None`, and
   dispatch one `Cmd::RevalidateList` into the async loop (pure seam:
   `entry_cmds(&Model)`). Deserialization failure is a cold entry, never an error.
2. **Cold entry is unchanged** (deliberate deviation from the fork, which shows
   an in-TUI loading placeholder): the existing pre-TUI blocking fetch runs
   byte-identically — including its stderr error contract and the E2 401
   re-auth message — and on success **writes the snapshot** before opening.
   Changing the cold path would break a pinned stderr contract for zero SWR
   benefit; the slow path a warm snapshot cannot help is out of R-E3's scope.
3. **Pure guards in `update` (fork BDR 0005's single-flight, adapted).**
   New model flag `revalidating` (distinct from "no content yet"), new
   `Msg::RevalidationLoaded / RevalidationFailed`:
   - `RevalidationLoaded` swaps the rows (selection clamped, not reset),
     restores `next_page_token`, clears `revalidating` — but is **ignored**
     when `revalidating` is already false (a newer user action won).
   - Submitting a search clears `revalidating`, so a late revalidation result
     never clobbers fresher search results.
   - `RevalidationFailed` keeps the painted list, clears the flag, and surfaces
     the message on the D4 status row (Error) — Unauthorized maps to the E2
     re-auth guidance.
   - Load-more while `revalidating` emits **no** `Cmd` — dropped, not queued.
4. **Snapshot writes are mine-scope only**, at the two success points: the cold
   pre-TUI fetch and the revalidation completion (shell-side, where the
   connection lives; the TUI core stays pure). Search results and load-more
   pages are never snapshotted.
5. **Subtle indicator**: while `revalidating`, the header bar shows a dim
   `refreshing…` (i18n'd) on its right side — the painted list is never blanked.

## Alternatives considered

- **Port the fork's cold path too (in-TUI loading placeholder).** Rejected: it
  restructures `fetch_and_run`'s error contract (pinned by tests, extended by
  E2) for a path SWR cannot accelerate anyway.
- **Snapshot search results per JQL.** Rejected: unbounded key space, and entry
  always lands on `mine`; the fork also snapshotted only its two list scopes.
- **Persist `next_page_token` in the snapshot.** Rejected: tokens expire
  server-side; a warm entry simply has no load-more until revalidation restores
  the cursor (and load-more is dropped while revalidating anyway).
- **Generation counters for late-result ordering.** Rejected as over-machinery:
  the single `revalidating` flag plus "search submit clears it" gives the same
  guarantee for the one overlap that exists.

## Consequences

**Positive:** warm `browse` paints instantly with the last-known list and
corrects itself moments later; the store layer needs zero changes; all guards
are pure and unit-testable headless.

**Accepted trade-offs:** the first frame may be briefly stale (bounded by
max-age + always-on revalidation); cold entry stays blocking (documented
deviation); a warm entry momentarily lacks the load-more cursor.

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-E3.
- BDR: [/bdr/0008-browse-entry-swr-behaviors.md](/bdr/0008-browse-entry-swr-behaviors.md) — the observable scenarios.
- Fork base: ADR 0017 + BDR 0011 (entry SWR), BDR 0005 (single-flight) in `active-collab-cli/docs`.
- ADR: [/adr/0008-browse-tui-async-event-loop.md](/adr/0008-browse-tui-async-event-loop.md) — the loop the revalidation runs on.
- Issue: [/issues/0036-e3-swr-first-paint-browse-entry.md](/issues/0036-e3-swr-first-paint-browse-entry.md)
