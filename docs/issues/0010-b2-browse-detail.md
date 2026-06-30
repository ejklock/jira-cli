---
type: Issue
title: "B2 — browse TUI: issue detail on Enter (cache-or-fetch, scroll, back)"
description: Enter on a list row pushes a Detail screen and emits a LoadDetail Cmd that reuses the get cache-or-fetch seam; the detail view renders summary/status/type/assignee/flattened description, is scrollable, and Esc/b pops back to the list preserving selection.
status: done
tracker:
tags: [tui, browse, phase2, detail]
timestamp: 2026-06-30T00:00:00Z
---

# B2 — browse TUI: issue detail on Enter

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R2 → [BDR 0006](/bdr/0006-browse-tui-interactions.md)
S2 (open detail) / S5 (back) → architecture [ADR 0007](/adr/0007-browse-tui-elm-architecture.md).
Stacks on the B1 list ([issue 0009](/issues/0009-b1-browse-list.md)).

## Context manifest

- **Read first:** `src/tui.rs` (B1 state — `Model { rows, selected }`, `Msg { Up, Down, Quit }`,
  `Cmd { Quit }`, pure `update`, `view`, the `run_tui`/`draw_loop` shell, async `fetch_and_run`),
  `src/commands.rs` (`load_issue` L458 — the private cache-or-fetch that `get_core` L409 uses;
  `IssueCache::new(cache.conn())` L430), `src/store/cache.rs` (`TaskCache::conn()` L98,
  `IssueCache` L22 with `read`/`write`), `src/models.rs` (`Issue` L25 — `summary`, `status`,
  `status_category`, `issue_type`, `assignee: Option<IssueAssignee>`, `description: Option<String>`
  ADF, `comments: Vec<IssueComment>`), `src/render.rs` (`adf_to_plain_text` L14 — the pure ADF
  flattener; reuse it to flatten `description` for display), `src/main.rs` (`dispatch_browse` L354,
  `dispatch_get` L276 — the `TaskCache::new(store.conn())` construction to mirror).
- **Expose the detail seam (no drift):** change `load_issue` in `src/commands.rs` to `pub(crate)`
  so the TUI reuses the EXACT cache-or-fetch path (`get`'s data seam, NOT a rendering `*_core`).
  Do not duplicate cache/fetch logic in `tui.rs`.
- **Thread the cache into `browse`:** `dispatch_browse` must build `TaskCache::new(store.conn())`
  (like `dispatch_get`) and pass `&cache` to `browse`. `browse`/`fetch_and_run` gain a
  `cache: &TaskCache` param. Keep `store` alive in `dispatch_browse` so the cache borrow is valid
  (bind `ResolvedInstance { store, instance }`, not `{ instance, .. }`).
- **TEA growth (still one file `src/tui.rs`):** add a screen stack —
  `enum Screen { List, Detail }`; grow `Model` with `screen: Screen`, `detail: Option<Issue>`
  (the loaded issue; `None` = loading/empty), and a detail scroll offset (`u16`). Grow `Msg` with
  `Select`, `Back`, `DetailLoaded(Box<Issue>)`; grow `Cmd` with `LoadDetail(String)` (the issue
  key). Keep `update` pure: `Select` (List, non-empty) sets `screen=Detail`, `detail=None`, emits
  `Cmd::LoadDetail(rows[selected].key)`; `DetailLoaded(issue)` stores `detail=Some(*issue)` and
  resets scroll; `Back` (Detail) sets `screen=List`, `detail=None` (selection is preserved because
  `selected` is untouched); `Up`/`Down` move the list selection on List and scroll on Detail
  (clamped, no panic). No async/I/O in `update`.
- **Execute `LoadDetail` in the shell (Humble Object):** the sync `draw_loop` runs the async
  `load_issue` via the captured `tokio::runtime::Handle` with
  `tokio::task::block_in_place(|| handle.block_on(load_detail(...)))` (the runtime is
  `rt-multi-thread`, so `block_in_place` is valid), then feeds the result back as
  `Msg::DetailLoaded`. A fetch error keeps the UI usable (return to List or show an empty detail
  notice — no crash, no broken terminal). This glue is the untested shell; the transitions are
  pure-tested.
- **Detail `view`:** when `screen == Detail`, render `summary`, `status` (+ `status_category`),
  `issue_type`, assignee (`display_name` or `t("Unassigned")`), and the flattened description
  (`adf_to_plain_text(&description)`), scrollable by the offset. `detail == None` → a brief
  loading/empty notice. Reuse existing i18n keys via `t()`; add NO new catalog keys.

## Vertical Demo

- **Given** a TTY, a configured instance, and a list with issues,
  **When** I press `Enter` on a row,
  **Then** the detail screen shows that issue's summary, status, type, assignee, and description;
  `↑`/`↓` scroll; `Esc`/`b` returns to the list with the same row still highlighted; `q` quits.
- **Edge:** **Given** the detail fetch fails (e.g. network/404),
  **When** I press `Enter`,
  **Then** the TUI stays usable (returns to the list or shows an empty-detail notice) — no crash,
  no broken terminal.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(Select)` on a non-empty list sets `screen=Detail` and emits `Cmd::LoadDetail(selected key)`; on an empty list it is a no-op (no Cmd) | test |
| AC2 | behavior | `update(Back)` from Detail sets `screen=List` and preserves `selected`; `update(DetailLoaded(issue))` stores the issue and resets the scroll | test |
| AC3 | behavior | The Detail `view` rendered to a ratatui `TestBackend` buffer shows the issue summary, status, and the flattened description; a `None` detail shows the loading/empty notice | test |
| AC4 | constraint | Detail load reuses the `pub(crate)` `load_issue` cache-or-fetch seam: a cache hit serves without network; a fetch error leaves the UI usable (wiremock) | test |
| AC5 | constraint | No superfluous comments / banners / commented-out code; cyclomatic ≤10 (≤8 new `update`/`view` arms) / cognitive within ceiling | command (comment_policy + complexity) |
| AC6 | constraint | Honors ADR 0007: `update`/detail-`view` pure and tested off-terminal; only the `block_on` glue is the untested shell; NO write path; `src/tui.rs` stays one file; `load_issue` reused (not duplicated) | inspection (Reviewer) |

## Out of scope

- Interactive **search** input — slice B3 (issue 0011).
- **Open-link / copy** affordances — slice B4 (issue 0012).
- Comment pagination / rich ADF (tables, panels) beyond the existing `adf_to_plain_text` output.
- In-loop async refresh of the list, in-TUI list pagination — deferred (PRD 0002 open questions).

## blocked_by

[0009](/issues/0009-b1-browse-list.md)
