---
type: Issue
title: "B1 — browse TUI: navigable issue list (mine)"
description: Introduce the Elm/TEA Model/Msg/Cmd in src/tui.rs, load my open issues via JiraClient::search (the mine JQL), and render a keyboard-navigable list with selection.
status: done
tracker:
tags: [tui, browse, phase2, list]
timestamp: 2026-06-30T00:00:00Z
---

# B1 — browse TUI: navigable issue list (mine)

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R1 → [BDR 0006](/bdr/0006-browse-tui-interactions.md)
S1/S7 → architecture [ADR 0007](/adr/0007-browse-tui-elm-architecture.md). Stacks on
the B0 skeleton ([issue 0008](/issues/0008-b0-browse-skeleton.md)).

## Context manifest

- **Read first:** `src/tui.rs` (B0 skeleton — `browse`/`run_tui`/`draw_loop`/`render_placeholder`),
  `src/commands.rs` (`MINE_JQL` L14, `DEFAULT_SEARCH_LIMIT` L13, `mine_core` L548 as the
  reference data path), `src/client.rs` (`JiraClient::search` L13, `GouqiJiraClient::new`
  L26), `src/models.rs` (`IssueRow` L51, `SearchResult` L61), `src/render.rs`
  (`render_issue_table` L169 — the column contract KEY·TYPE·STATUS·ASSIGNEE·SUMMARY +
  the `t()` header keys to mirror).
- **Expose the shared JQL (no drift):** change `const MINE_JQL` and
  `const DEFAULT_SEARCH_LIMIT` in `src/commands.rs` to `pub(crate)` so the TUI uses the
  EXACT same mine query — do not redefine the JQL string in `tui.rs`.
- **Introduce the TEA core in `src/tui.rs`** (still one file): `struct Model { rows:
  Vec<IssueRow>, selected: usize }`, `enum Msg { Up, Down, Quit }`, `enum Cmd { Quit }`,
  and a pure `fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>)`. `Down`/`Up` move
  `selected` and **clamp** at both ends (no wrap, never out of range, no-op on an empty
  list); `Quit` emits `Cmd::Quit`. No async, no I/O in `update`.
- **Load synchronously before the loop (PRD 0002 open question):** in `browse` (async),
  after the TTY guard, build the client (`GouqiJiraClient::new(instance)`) and
  `client.search(MINE_JQL, DEFAULT_SEARCH_LIMIT).await`. On error, print the error to
  stderr and return non-zero BEFORE entering raw mode (mirror `mine_core`'s error
  handling). On success, pass the `Vec<IssueRow>` into `run_tui` to seed the `Model`.
- **Render:** a `fn view(model: &Model, frame: &mut Frame)` drawing a header row
  (`t("KEY")` … `t("SUMMARY")`) and the issue rows (key / issue_type / status /
  assignee-or-`t("Unassigned")` / summary), highlighting `selected`. Empty list →
  `t("No issues.")`. The footer hint shows navigation + quit. `draw_loop` maps
  `KeyCode::Up`/`Down` → `Msg`, `q`/Ctrl+C → `Msg::Quit`, calls `update`, redraws via
  `view`, and breaks when `Cmd::Quit` is returned.
- **Clean up B0 nit:** `run_tui` had a redundant `io::stdout()` local — draw via the
  single backend handle.

## Vertical Demo

- **Given** a TTY and a configured instance with assigned issues,
  **When** I run `jira browse`,
  **Then** my open issues render as a navigable list; `↑`/`↓` move the highlight
  (clamped at the ends); `q` exits cleanly.
- **Edge:** **Given** no assigned open issues, **When** I run `jira browse`, **Then**
  the list area shows `No issues.` and `q` still exits.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(model, Down)` increments `selected` and clamps at the last row; `update(model, Up)` decrements and clamps at 0; both no-op on an empty list | test |
| AC2 | behavior | `update(model, Quit)` returns `Cmd::Quit`; arrow Msgs never return `Cmd::Quit` | test |
| AC3 | behavior | `view` rendered to a ratatui `TestBackend` buffer shows the header columns and each issue's KEY; an empty model shows `No issues.` | test |
| AC4 | constraint | The mine list is fetched via `JiraClient::search` with the shared `MINE_JQL`; a fetch error exits non-zero before raw mode (wiremock or a stub `JiraClient`) | test |
| AC5 | constraint | No superfluous comments / banners / commented-out code; only why-comments; cyclomatic ≤10 (≤8 new) / cognitive within ceiling | command (comment_policy + complexity) |
| AC6 | constraint | Honors ADR 0007: `update`/`view` are pure and tested off-terminal; only the raw-mode loop is the untested shell; NO write path; `src/tui.rs` stays one file | inspection (Reviewer) |

## Out of scope

- Issue **detail** on Enter — slice B2 (issue 0010).
- Interactive **search** input — slice B3 (issue 0011).
- **Open-link / copy** affordances — slice B4 (issue 0012).
- Async refresh during the loop, in-TUI pagination — deferred (PRD 0002 open questions).

## blocked_by

[0008](/issues/0008-b0-browse-skeleton.md)
