---
type: BDR
title: "Browse-entry SWR: paint the cached list instantly, always revalidate, guard the swap"
description: Observable behavior of entering browse — a warm snapshot paints immediately while a revalidation runs in the loop; a cold entry keeps the blocking fetch and seeds the snapshot; late/failed revalidations never clobber newer state. Includes the Test Design matrix.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, cache, swr, entry, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0008. Browse-entry SWR behaviors

Realizes [ADR 0016](/adr/0016-swr-first-paint-browse-entry.md) (port of
fork-base BDR 0011 + the single-flight rule of fork-base BDR 0005).

## Definitions

- **Snapshot** — the `task_list_cache` row for `("mine", instances_key)`: the
  serialized `Vec<IssueRow>` plus `fetched_at`.
- **Warm entry** — a snapshot exists for the instance set, within max-age
  (7 days). **Cold entry** — no snapshot, over-max-age, or undeserializable.
- **`revalidating`** — model flag: cached content is shown while a fresh fetch
  is in flight.

## Scenarios

### S1 — Warm entry paints the cached list immediately and revalidates
**Given** a warm snapshot for the target instance,
**When** the operator enters `browse`,
**Then** the TUI opens without a blocking fetch, the first frame shows the
snapshot rows with `revalidating: true`, and exactly one `Cmd::RevalidateList`
is dispatched (`entry_cmds`).

### S2 — Revalidation swaps in fresh data and rewrites the snapshot
**Given** a warm entry showing the snapshot (`revalidating: true`),
**When** the fresh fetch completes,
**Then** the rows are replaced (selection clamped to the new length, not
reset), `next_page_token` is restored, `revalidating` clears, **and** the
snapshot is rewritten at the shell seam.

### S3 — Cold entry keeps the blocking fetch and seeds the snapshot
**Given** no usable snapshot,
**When** the operator enters `browse`,
**Then** the pre-TUI fetch runs exactly as before (stderr error contract —
including the E2 401 message — byte-identical) and, on success, the snapshot
is written before the TUI opens with `revalidating: false`.

### S4 — A late revalidation never clobbers a newer search
**Given** a warm entry with a revalidation in flight,
**When** the operator submits a search (which clears `revalidating`) and the
stale revalidation result arrives afterwards,
**Then** the `RevalidationLoaded` is ignored — the search results stay.

### S5 — Revalidation failure keeps the painted list
**Given** a warm entry with a revalidation in flight,
**When** the fetch fails (including a 401),
**Then** the rows remain, `revalidating` clears, and the message (the E2
re-auth guidance for 401) appears on the status row (Error style).

### S6 — Single-flight: load-more is dropped while revalidating
**Given** `revalidating: true`,
**When** the operator triggers load-more,
**Then** no `Cmd` is emitted and the model is unchanged (dropped, not queued).

### S7 — Snapshot is isolated per instance set and TTL-bounded
**Given** a snapshot for instance set A (or one older than max-age),
**When** entering with instance set B (or after the TTL),
**Then** it is a cold entry (already pinned by the existing `TaskListCache`
store tests; referenced, not re-implemented).

### S8 — Revalidating indicator
**Given** `revalidating: true`,
**Then** the header bar shows a dim i18n'd `refreshing…` on its right side;
it disappears once revalidation settles. The list is never blanked.

## Test Design

| Scenario | Level | Technique | Instrument / assertion |
|---|---|---|---|
| S1 | unit (pure model) | example | model seeded warm → rows non-empty, `revalidating=true`; `entry_cmds` yields exactly `[Cmd::RevalidateList]`; cold seed yields `[]` |
| S2 | unit (pure model) | example | `update(RevalidationLoaded)` swaps rows, clamps `selected`, sets token, clears flag; shell snapshot write asserted at the store seam (temp SQLite) |
| S3 | integration (wiremock + temp SQLite) | example | cold `fetch_and_run` writes the snapshot on success; failure output byte-identical to pre-E3 |
| S4 | unit (pure model) | example | search submit clears `revalidating`; subsequent `RevalidationLoaded` is a no-op |
| S5 | unit (pure model) | example | `update(RevalidationFailed)` keeps rows, clears flag, sets status Error |
| S6 | unit (pure model) | example | load-more msg with `revalidating=true` → empty `Vec<Cmd>`, model unchanged |
| S7 | unit (store) | boundary | existing `tests/unit/store/cache.rs` TaskListCache TTL/isolation tests (referenced) |
| S8 | unit (TestBackend) | example | frame with `revalidating=true` contains the dim `refreshing…`; absent when false; pt-BR renders under LANG_MUTEX |

## References

- ADR: [/adr/0016-swr-first-paint-browse-entry.md](/adr/0016-swr-first-paint-browse-entry.md)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-E3.
- Issue: [/issues/0036-e3-swr-first-paint-browse-entry.md](/issues/0036-e3-swr-first-paint-browse-entry.md)
