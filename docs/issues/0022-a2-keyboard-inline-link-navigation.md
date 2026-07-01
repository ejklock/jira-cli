---
type: Issue
title: "A2 — keyboard inline-link navigation in the browse TUI detail"
description: Make the description's inline body links openable by keyboard in the browse TUI detail. Tab cycles a focused link, Enter opens it (reusing Cmd::OpenUrl), and view_detail highlights the focused link. Builds on A1's adf_to_rich (the retained link href). Second read-only slice of the active-collab-cli parity program (Group A), adapting AC's Ctrl/Cmd+click to the keyboard contract.
status: done
tracker:
tags: [tui, browse, links, keyboard, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# A2 — keyboard inline-link navigation in the browse TUI detail

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R4 (read affordances), realizing
[ADR 0011](/adr/0011-keyboard-inline-link-navigation-browse-detail.md) over
[ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md)'s `adf_to_rich`.
Group A parity slice #2; reuses A1's retained link href.

## Context manifest

- **Read first:** `src/tui/model.rs` — `Model` (now has rows/selected/screen/detail/
  detail_scroll/search/error/base_url/jql/next_page_token), `Msg`, `Cmd` (has
  `OpenUrl(String)`), `update` + `update_*`. Key handlers to touch: `update_select`
  (L118 — List→LoadDetail; extend for Detail→open focused link), `update_detail_loaded`
  (L141 — populate links), `update_back` (L132 — clear links), and `update_open_link`
  (L262 — unchanged, still opens the issue's own URL).
- `src/tui/shell.rs` — `map_key_in_normal_mode` (L131): add `KeyCode::Tab =>
  Some(Msg::FocusNextLink)`. It is the shared List/Detail normal-mode map; `update`
  guards by screen. (Do NOT refactor this fn's structure — its pre-existing complexity
  is a separate ticket.)
- `src/tui/view.rs` — `view_detail` renders the description via `adf_to_rich` mapped to
  ratatui spans (A1). Add focused-link highlighting: count link spans in render order,
  and when the running link index == `detail_focused_link`, add `Modifier::REVERSED` on
  top of the link style.
- `src/render.rs` — `adf_to_rich(raw) -> Vec<RichLine>` (A1); `RichStyle.link:
  Option<String>` carries the href. `issue_browse_url` (used by `update_open_link`).
- `src/models.rs` — `Issue.description: Option<String>` (raw ADF).
- `tests/unit/tui.rs` — `update`/`TestBackend` tests + fixture builders (incl.
  `make_issue_with_styled_description` and the `doc`/`paragraph`/`marked_text`/`link_mark`
  helpers from A1's round-2 refactor — reuse them).

## Approach (decided — see ADR 0011)

- **Model** (`src/tui/model.rs`): add `detail_links: Vec<String>` and
  `detail_focused_link: Option<usize>`. A private pure helper collects the description's
  inline hrefs in order from `adf_to_rich` (e.g. `fn description_link_hrefs(issue: &Issue)
  -> Vec<String>` iterating `RichSpan.style.link`). Keep it in `model.rs` (it already
  imports `crate::render`); do NOT add a new render.rs public fn (keeps the slice ≤4 files).
- `update_detail_loaded`: set `detail_links` from the loaded issue's description;
  `detail_focused_link = (!detail_links.is_empty()).then_some(0)`.
- `update_back`: clear `detail_links = vec![]`, `detail_focused_link = None`.
- New `update_focus_next_link`: if `screen == Detail` and `!detail_links.is_empty()`,
  advance `detail_focused_link` wrapping (`Some((i + 1) % len)`); else no-op (empty cmds).
- `update_select`: List → `Cmd::LoadDetail` (unchanged); Detail → if
  `detail_focused_link == Some(i)`, `vec![Cmd::OpenUrl(detail_links[i].clone())]`; else no-op.
- `Msg`: add `FocusNextLink`. Reuse `Cmd::OpenUrl` (no new Cmd).
- **Shell**: `map_key_in_normal_mode` add `KeyCode::Tab => Some(Msg::FocusNextLink)`.
- **View** (`view_detail`): while emitting description spans, track a link counter; when
  a span has a link and its index == `detail_focused_link`, add `Modifier::REVERSED`.
  Highlight only in Detail with a focused link.
- Read-only; `o` (issue URL) and `y` (copy key) unchanged; no comments in detail (A4).

## Vertical Demo

- **Given** `jira browse` and an issue whose description contains two inline links,
- **When** I open its detail and press `Tab`,
- **Then** the first link is highlighted; `Tab` again moves the highlight to the second
  (and wraps back); pressing `Enter` opens the highlighted link's URL in the browser.
- **And** `o` still opens the issue's own `/browse/KEY` URL, and an issue with no links
  makes `Tab`/`Enter` no-ops (nothing crashes, nothing opens).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(DetailLoaded)` populates `detail_links` with the description's inline hrefs in order and focuses index 0 when links exist (None when none); `update(Back)` clears both | test (unit) |
| AC2 | behavior | `update(FocusNextLink)` on Detail with links advances the focus wrapping; with no links (or on List) it is a no-op | test (unit) |
| AC3 | behavior | `update(Select)` on Detail with a focused link emits `Cmd::OpenUrl(that href)`; on Detail with no focused link it is a no-op; on List it still emits `Cmd::LoadDetail` (unchanged) | test (unit) |
| AC4 | behavior | `view_detail` renders the focused inline link with `Modifier::REVERSED` (on top of its underline); a non-focused link has underline without reversed — asserted via `TestBackend` cell style | test (TestBackend) |
| AC5 | constraint | `update` stays pure (no I/O); the open is `Cmd::OpenUrl` dispatched by the shell; `o` (issue URL) + `y` (copy key) behavior unchanged; `map_key_in_normal_mode` structure not refactored (only the `Tab` arm added) | inspection |
| AC6 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; cyclomatic ≤10 (≤8 new fns) / cognitive within ceiling; a mutant that breaks the focus-advance wrap or the focused-href selection is killed by AC2/AC3; no superfluous comments/banners/commented-out code | command (clippy + fmt + comment_policy + complexity) |

## Out of scope

- `inlineCard`/smartlink nodes; auto-scrolling to an off-screen focused link (refinements).
- Links inside comments (comments are not shown until A4; link nav there rides on A4).
- Mouse activation (Group B, parked). Any change to `update_open_link` (issue URL) or the CLI.

## blocked_by

- [0021](/issues/0021-a1-styled-adf-detail-rendering.md) (A1 — provides `adf_to_rich` + the retained link href)
