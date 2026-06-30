---
type: Issue
title: "B3 — browse TUI: interactive JQL search"
description: A key opens a JQL input line; submitting runs the query via JiraClient::search and replaces the list; an invalid JQL (400) sets an inline error banner and keeps the prior list with the UI usable (no crash, no broken terminal).
status: done
tracker:
tags: [tui, browse, phase2, search]
timestamp: 2026-06-30T00:00:00Z
---

# B3 — browse TUI: interactive JQL search

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R3 → [BDR 0006](/bdr/0006-browse-tui-interactions.md)
S3 (search) / S6 (invalid JQL) → architecture [ADR 0007](/adr/0007-browse-tui-elm-architecture.md).
Stacks on the B1 list ([issue 0009](/issues/0009-b1-browse-list.md)).

## Context manifest

- **Read first:** `src/tui.rs` (B2 state — `Model { rows, selected, screen, detail, detail_scroll }`,
  `Screen { List, Detail }`, `Msg { Up, Down, Select, Back, DetailLoaded, Quit }`,
  `Cmd { Quit, LoadDetail }`, pure `update`/`update_*` helpers, `map_key_to_msg`, `view`/`view_list`/
  `view_detail`, the `run_tui`/`draw_loop` shell with the `block_in_place`+`block_on` glue, async
  `fetch_and_run` + `load_detail`), `src/client.rs` (`JiraClient::search` L13 — the same seam B1's
  list uses; returns `Result<SearchResult>`), `src/commands.rs` (`DEFAULT_SEARCH_LIMIT`, `MINE_JQL`
  already `pub(crate)`), `src/models.rs` (`IssueRow` / `SearchResult { issues }`).
- **Reuse the existing search seam (no new path):** submitting a JQL runs the SAME
  `client.search(jql, DEFAULT_SEARCH_LIMIT)` the list already uses — build
  `GouqiJiraClient::new(instance)` per submit (mirror `load_detail`'s per-call construction). Do NOT
  add a new client method or a `*_core` call.
- **TEA growth (still one file `src/tui.rs`):** add a search-input state and an error banner to the
  List screen. Grow `Model` with `search: Option<String>` (`Some` = the input line is active and
  holds the typed query; `None` = inactive) and `error: Option<String>` (inline banner; `None` =
  none). Grow `Msg` with `OpenSearch`, `SearchInput(char)`, `SearchBackspace`, `SubmitSearch`,
  `CancelSearch`, `ListLoaded(Box<Vec<IssueRow>>)`, `LoadFailed(String)`. Grow `Cmd` with
  `LoadList(String)` (the JQL). Keep `update` pure:
  - `OpenSearch` (screen==List) → `search=Some(String::new())`, clear `error`.
  - `SearchInput(c)` → if `search` active, push `c`.
  - `SearchBackspace` → if `search` active, pop the last char.
  - `SubmitSearch` → if `search` is `Some(q)` and `q` non-empty, emit `vec![Cmd::LoadList(q)]` (keep
    the prior list visible until the result arrives); empty query → no-op.
  - `CancelSearch` → `search=None` (keep the current list + clear nothing else).
  - `ListLoaded(rows)` → `rows=*rows`, `selected=0`, `search=None`, `error=None`.
  - `LoadFailed(msg)` → `error=Some(msg)`, `search=None`, **rows preserved** (the prior list stays).
  No async/I/O in `update`.
- **Execute `LoadList` in the shell (Humble Object):** in `draw_loop`, handle `Cmd::LoadList(jql)`
  by running the async search synchronously via the captured `Handle`:
  `tokio::task::block_in_place(|| handle.block_on(run_search(&instance, &jql)))` where
  `async fn run_search` builds `GouqiJiraClient::new(instance)` + `client.search(&jql,
  DEFAULT_SEARCH_LIMIT)`. `Ok(result)` → apply `Msg::ListLoaded(Box::new(result.issues))`; `Err(e)`
  → apply `Msg::LoadFailed(e.to_string())` (an invalid JQL surfaces as a 400 → Err). The UI stays
  usable; the terminal is never left broken. This glue is the untested shell.
- **Key mapping (`map_key_to_msg`):** when `search` is active, route printable chars →
  `SearchInput(c)`, Backspace → `SearchBackspace`, Enter → `SubmitSearch`, Esc → `CancelSearch`,
  and DO NOT treat `q`/arrows as navigation (so the user can type `q` in a query). When `search` is
  inactive, a dedicated key (`/`) → `OpenSearch`, and the B1/B2 navigation mapping stays.
- **`view` (List screen):** when `search` is `Some(q)`, render an input line showing the typed
  query (e.g. a `JQL>` prompt + `q`); when `error` is `Some(msg)`, render an inline error banner
  above/below the list while keeping the list rows visible. Reuse existing `t()` keys where they
  fit; a plain literal is acceptable for a transient prompt/banner with no existing key (match the
  B2 `LOADING_NOTICE` precedent).

## Vertical Demo

- **Given** a TTY and the list screen,
  **When** I press `/`, type a valid JQL, and press `Enter`,
  **Then** the list is replaced by the query results; `q` quits.
- **Edge:** **Given** I submit an invalid JQL,
  **When** the server returns 400,
  **Then** an inline error banner appears, the prior list is still shown, and the TUI stays usable
  (no crash, no broken terminal); `Esc` cancels the input.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(SubmitSearch)` with a non-empty active query emits `Cmd::LoadList(jql)`; an empty query is a no-op | test |
| AC2 | behavior | `update(LoadFailed(msg))` sets the error banner and PRESERVES the prior `rows`; `update(ListLoaded(rows))` replaces rows, resets `selected`, clears `search`+`error` | test |
| AC3 | behavior | With `search=Some(q)`, the List `view` rendered to a ratatui `TestBackend` buffer shows the typed query; with `error=Some(msg)` the banner text appears and the list rows are still rendered | test |
| AC4 | constraint | Submit runs the user JQL via `JiraClient::search`; a 400 (invalid JQL) surfaces as `LoadFailed` keeping the list (wiremock) | test |
| AC5 | constraint | No superfluous comments / banners / commented-out code; cyclomatic ≤10 (≤8 new `update_*`/`view` arms) / cognitive within ceiling | command (comment_policy + complexity) |
| AC6 | constraint | Honors ADR 0007: `update`/`view` pure and tested off-terminal; only the `block_on` search glue is the untested shell; NO write path; `search` reuses `JiraClient::search` (no new method); `src/tui.rs` stays one file | inspection (Reviewer) |

## Out of scope

- **Open-link / copy** affordances — slice B4 (issue 0012).
- Search history, autocomplete, saved filters, multi-page result paging.
- Re-running `mine` from inside search (the `r refresh` affordance is a later concern).

## blocked_by

[0009](/issues/0009-b1-browse-list.md)
