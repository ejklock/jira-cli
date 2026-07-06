---
type: Issue
title: "E3 — SWR first paint on browse entry (snapshot + revalidate + guards)"
description: Wire the inherited TaskListCache so a warm browse entry opens the TUI instantly from the last mine snapshot with revalidating:true and one Cmd::RevalidateList; cold entry keeps the blocking fetch and seeds the snapshot; pure update guards (late-result, failure-keeps-list, load-more single-flight) + dim header refreshing… indicator.
status: open
labels: [tui, performance, cache, swr, parity]
blocked_by: 0033
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## E3 — SWR first paint on browse entry

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-E3 per
[ADR 0016](/adr/0016-swr-first-paint-browse-entry.md), behaviors pinned by
[BDR 0008](/bdr/0008-browse-entry-swr-behaviors.md) S1–S8.

Store layer needs zero changes: `TaskListCache` + its TTL/isolation tests were
carried over in the fork scaffold. The slice wires it at `fetch_and_run`,
adds the pure `revalidating` state machine to `model.rs`, the shell-side
revalidation dispatch + snapshot writes, and the header indicator.
