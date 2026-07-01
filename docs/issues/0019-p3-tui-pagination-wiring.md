---
type: Issue
title: "P3 — browse TUI pagination wiring: load-more appends the next page"
description: Wire in-TUI pagination onto the P2 client primitive and the P1 async loop. The list Model tracks the active jql + next_page_token; an explicit load-more action (key 'n', enabled while a token is pending) fetches the next page via search_page and appends the rows, preserving selection and advancing the token; view_list shows a load-more affordance.
status: open
tracker:
tags: [tui, browse, phase2, pagination]
timestamp: 2026-06-30T00:00:00Z
---

# P3 — browse TUI pagination wiring: load-more appends the next page

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) "in-TUI paging" (resolved),
[ADR 0009](/adr/0009-tui-list-pagination.md). Verified by
[BDR 0006](/bdr/0006-browse-tui-interactions.md) S8 (load-more append) + the load-more
append / no-op unit rows in its Test Design.

## Context manifest

- **Read first:** `src/tui/model.rs` — `Model` (rows/selected/screen/detail/detail_scroll/
  search/error/base_url), `Msg` (has `ListLoaded(Vec<IssueRow>)`, `LoadFailed(String)`,
  `Select`, `Down`, …), `Cmd` (`LoadList`/`LoadDetail`/`OpenUrl`/`CopyToClipboard`/`Quit`),
  and the `update_*` helpers (`update_submit_search` emits `Cmd::LoadList(jql)`;
  `update_list_loaded` resets rows/selected/search/error).
- `src/tui/view.rs` — `view_list` renders the table + a footer hint (the `has_search_bar ?
  … : "↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"` line).
- `src/tui/shell.rs` — after P1 this is the async loop; `run_search` fetches a list;
  `dispatch_cmd` spawns effects and returns `Msg`s. `map_key_in_normal_mode` maps keys.
- `src/client.rs` — P2 added `JiraClient::search_page(jql, max_results, page_token)` and
  `SearchResult.next_page_token`; `MINE_JQL`/`DEFAULT_SEARCH_LIMIT` live in `src/commands.rs`.
- `tests/unit/tui.rs` — existing `update`/`TestBackend` tests, incl.
  `update_list_loaded_replaces_rows_resets_selected_clears_search_and_error` which
  constructs `Msg::ListLoaded(new_rows)` — this call site changes when the payload gains a
  token (update it in this slice).

## Approach (decided — see ADR 0009)

- **Model** gains `jql: String` (the active list query; initialized to `MINE_JQL` at
  construction) and `next_page_token: Option<String>`.
- **Msg payload** carries the token: `ListLoaded(Vec<IssueRow>, Option<String>)` and a new
  `MoreLoaded(Vec<IssueRow>, Option<String>)`; add `Msg::LoadMore`. Add `Cmd::LoadMore(String, String)`
  (jql, token).
- **Pure `update` transitions (model.rs):**
  - `update_submit_search`: set `model.jql = q.clone()` before emitting `Cmd::LoadList(q)`
    (so the next page repeats the same query). The initial `mine` load uses the constructed
    `jql`.
  - `update_list_loaded(rows, token)`: replace rows, reset selected/search/error, set
    `next_page_token = token` (a fresh list resets the paging cursor).
  - `update_more_loaded(rows, token)`: **append** rows to the existing list (selection
    preserved), set `next_page_token = token`.
  - `update_load_more`: if `screen == List` and `next_page_token.is_some()`, emit
    `Cmd::LoadMore(jql.clone(), token.clone())`; else no-op (empty cmds).
- **Shell (shell.rs):** `run_search` returns rows + token (thread `SearchResult`); the
  `LoadList` effect sends `ListLoaded(rows, token)`. Add a `LoadMore` dispatch arm that
  spawns `search_page(jql, DEFAULT_SEARCH_LIMIT, &token)` → `MoreLoaded(rows, token)` (or
  `LoadFailed` on error). `map_key_in_normal_mode`: add `KeyCode::Char('n') => Some(Msg::LoadMore)`.
- **View (view.rs):** in the normal-list footer, show a load-more affordance (e.g. append
  `  n more` / a translated "more" hint) **only when `next_page_token.is_some()`**; the
  affordance disappears on the last page.
- Update the existing `tests/unit/tui.rs` `ListLoaded` construction to the 2-arg payload;
  add unit tests: load-more appends + preserves selection + advances token; load-more with
  no pending token is a no-op; and a `TestBackend` assertion that the "more" affordance shows
  only when a token is pending. (search_page wiremock coverage lives in P2.)

## Vertical Demo

- **Given** `jira browse` whose `mine` result has more than one page,
  **When** I press `n`,
  **Then** the next page's rows are appended below the current ones, my selection is
  preserved, and the footer keeps showing "more" until the last page (after which `n` is a
  no-op and the affordance is gone).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(LoadMore)` with `screen==List` and a pending `next_page_token` emits `Cmd::LoadMore(jql, token)`; with no token (or on Detail) it emits no Cmd | test |
| AC2 | behavior | `update(MoreLoaded(rows, token))` appends rows to the existing list (selection preserved) and sets `next_page_token=token`; `update(ListLoaded(rows, token))` replaces rows, resets selection, and sets the token | test |
| AC3 | behavior | `view_list` shows the load-more affordance only when `next_page_token.is_some()` (present with a pending token, absent on the last page) — asserted via `TestBackend` | test (TestBackend) |
| AC4 | constraint | `update` stays pure (no I/O); the shell's `LoadMore` effect is a spawned `search_page` returning `MoreLoaded`; navigation itself does not trigger hidden I/O beyond the explicit `n` | inspection |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; cyclomatic ≤10 / cognitive within ceiling; a surviving mutant on the append/token-advance logic is a fail | command (clippy + fmt + comment_policy + complexity) |
| AC6 | constraint | i18n: any new footer chrome ("more") goes through `t()` (Jira data never translated); en output stays identity | inspection |

## Out of scope

- Auto infinite-scroll (fetch on nearing the end) — ADR 0009 chose explicit load-more;
  auto-scroll is a later layer on this primitive.
- Detail-view pagination of comments; per-page caching of list results (PRD open item).
- Any client-seam change (done in P2).

## blocked_by

- [0017](/issues/0017-p1-async-event-loop.md) (P1 async loop — LoadMore rides it)
- [0018](/issues/0018-p2-pagination-client-seam.md) (P2 — provides `search_page` + `next_page_token`)
