---
type: Issue
title: "A4 — read-only comments in the browse TUI detail (styled ADF + j/k scroll)"
description: Show the issue's comments in the browse TUI detail, styled via adf_to_rich (A1) and scrollable in the existing detail paragraph. Add j/k as vim scroll aliases for Up/Down. Purely presentation + keymap — comments are already fetched into Model.detail.comments; no client/model/data change. Fourth read-only slice of the active-collab-cli parity program (Group A).
status: done
tracker:
tags: [tui, browse, comments, adf, keyboard, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# A4 — read-only comments in the browse TUI detail (styled ADF + j/k scroll)

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) (browse TUI detail), realizing
[ADR 0012](/adr/0012-comments-in-browse-tui-detail.md) over
[ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md)'s `adf_to_rich`.
Group A parity slice #4. Rides on issue 0023's `view.rs` hygiene (clean terrain).

## Context manifest

- **Read first:** `src/tui/view.rs` — `view_detail` (L174–239) builds a `Vec<Line>`
  (key/summary/status/type/assignee/`Description:` + `description_lines_to_ratatui`) fed to a
  single `Paragraph` with `.scroll((model.detail_scroll, 0))`. The rich→ratatui mapping helpers
  `rich_line_to_ratatui` (L274) and `span_style` (L289) are reusable; the detail footer string
  is at L186.
- `src/render.rs` — `render_issue_human` (L423) + `render_comment_human` (L485) already render
  comments for the CLI: section header `{t("Comments")}:`, per comment `  [{author}] {created}`
  then body (`adf_to_plain_text`), author fallback `t("Unknown")`, created fallback `""`. Mirror
  this layout (the header format + the i18n keys), but render the **body via `adf_to_rich`**
  (styled), not plain text. `adf_to_rich` is at L171.
- `src/models.rs` — `IssueComment { id, author: Option<String>, body: String, created:
  Option<String>, updated }` (L14); `Issue.comments: Vec<IssueComment>` (L39) is already
  populated on fetch. **Do not touch models.rs / client.rs / the fetch path.**
- `src/tui/shell.rs` — `map_key_in_normal_mode` (L133–149): screen-agnostic key map;
  `Up`→`Msg::Up`, `Down`→`Msg::Down` already present (L135–136). Add `j`/`k` arms only.
- `src/tui/model.rs` — `update_up`/`update_down` already scroll `detail_scroll` on the Detail
  screen and move selection on List. **No new Msg/Cmd/Model state** — j/k reuse `Msg::Down`/`Up`.
- `locales/pt_BR.json` — already has `"Comments": "Comentários"`, `"Unknown": "Desconhecido"`,
  and the detail footer `"↑/↓ scroll  Esc/b back  q quit": "↑/↓ rolar  Esc/b voltar  q sair"`
  (L113). Only the footer string changes (to advertise `j/k`).
- `tests/unit/tui.rs` — TUI render + shell-map tests. **Every language-dependent render test
  must lock `crate::i18n::LANG_MUTEX`** (issue 0023 discipline — do not regress the race fix).

## Approach (decided — see ADR 0012)

- **`view_detail`:** after extending `lines` with the description, append the comment section
  when `issue.comments` is non-empty: a blank `Line::from("")`, a `Line::from(format!("{}:",
  t("Comments")))` header, then per comment a `[{author}] {created}` header line (author →
  `t("Unknown")`, created → `""`) followed by the body's `adf_to_rich` lines mapped via
  `rich_line_to_ratatui(line, None, &mut counter)` (no focused link — comments aren't in
  `detail_links`), then a trailing blank line. When `comments` is empty, append nothing.
- **Extract** a pure helper `fn detail_comment_lines(issue: &Issue) -> Vec<Line<'static>>`
  building that section, so `view_detail` stays within the complexity ceiling (do not regress
  issue 0023). `view_detail` just `lines.extend(detail_comment_lines(issue))`.
- **Shell:** add `KeyCode::Char('j') => Some(Msg::Down)` and `KeyCode::Char('k') =>
  Some(Msg::Up)` to `map_key_in_normal_mode`. No other change to that fn.
- **Footer:** change the `view_detail` footer to `t("↑/↓ j/k scroll  Esc/b back  q quit")` and
  add the pt_BR entry `"↑/↓ j/k scroll  Esc/b back  q quit": "↑/↓ j/k rolar  Esc/b voltar  q sair"`.
  Update the existing footer assertion(s) accordingly. (The old footer key may remain an orphan
  in the catalog — do not spend effort removing it.)
- Read-only; `o` (issue URL), `y` (copy key), Tab (description link focus), `n` (load more)
  unchanged. No comment writes.

## Vertical Demo

- **Given** `jira browse` and an issue that has two comments with styled ADF bodies,
- **When** I open its detail,
- **Then** below the description I see a `Comments:` section, each comment showing
  `[author] created` and its styled body (bold/italic/code/links preserved via `adf_to_rich`),
- **And** pressing `j`/`k` (or `↓`/`↑`) scrolls down/up through the description and the comments,
- **And** an issue with **no** comments shows the detail with no `Comments:` section (nothing
  extra rendered, no crash).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `view_detail` renders, below the description, a `{t("Comments")}:` header and for each comment a `[{author}] {created}` header line (author→`t("Unknown")` when absent) plus its `adf_to_rich`-styled body; asserted for a 2-comment issue via `TestBackend` | test (TestBackend) |
| AC2 | behavior | A comment body's inline ADF marks render styled (e.g. a bold run carries `Modifier::BOLD`, a link run carries `UNDERLINED`) via the reused `rich_line_to_ratatui`; asserted via `TestBackend` cell style | test (TestBackend) |
| AC3 | behavior | An issue with `comments == []` renders the detail with **no** `Comments:` header line (negative assertion) and does not panic | test (TestBackend) |
| AC4 | behavior | `map_key_in_normal_mode('j') == Some(Msg::Down)` and `('k') == Some(Msg::Up)`; on the Detail screen `update(Down)`/`update(Up)` change `detail_scroll` (existing behavior, unchanged) | test (unit) |
| AC5 | constraint | No new `Msg`/`Cmd`/`Model` field; `models.rs`/`client.rs`/the fetch path and `view_detail`'s description/link-focus behavior are untouched; `view_detail` complexity stays ≤12 cognitive / ≤10 cyclomatic via the extracted `detail_comment_lines` helper; every language-dependent render test locks `LANG_MUTEX` | inspection + command (complexity) |
| AC6 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; a mutant that drops the comment body or the author fallback is killed by AC1; no superfluous comments/banners/commented-out code | command (clippy + fmt + comment_policy + test) |

## Out of scope

- Per-comment focus / highlight / jump-to-next-comment (scroll-into-view) — deferred to A5
  (needs a shared pure line-offset helper between model and view).
- Comment link navigation via Tab (stays description-only); `inlineCard`/smartlink nodes.
- Comment **writes** (create/edit/delete) — parked (Group C, needs a constitution amendment).
- Any change to `models.rs`, `client.rs`, the fetch path, or the CLI `get` rendering.

## blocked_by

- [0021](/issues/0021-a1-styled-adf-detail-rendering.md) (A1 — `adf_to_rich`)
- [0023](/issues/0023-tui-test-hygiene-and-view-list-complexity.md) (clean `view.rs` terrain + LANG_MUTEX discipline)
