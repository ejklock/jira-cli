---
type: ADR
title: "Read-only comments in the browse TUI detail (styled ADF + j/k scroll)"
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, comments, adf, i18n, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# ADR 0012 — Read-only comments in the browse TUI detail (styled ADF + j/k scroll)

## Status

Accepted.

## Context

The browse TUI detail ([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)) currently
renders only the issue's key, summary, status/type/assignee, and the styled description
([ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md)). It shows **no comments**,
even though every piece needed already exists end-to-end:

- `Issue.comments: Vec<IssueComment>` (`src/models.rs`) is already populated on fetch —
  `map_gouqi_issue` → `map_comments` (`src/client.rs`) fills it, so `Model.detail` already
  carries the comments after `DetailLoaded`. No client/model/data change is needed.
- The CLI `get` command already renders comments (`render_issue_human` →
  `render_comment_human` in `src/render.rs`) with a `t("Comments")` section header and a
  `[{author}] {created}` + body-per-comment layout, using `adf_to_plain_text` for the body.
- `view_detail` builds one `Vec<Line>` fed to a single scrollable `Paragraph`
  (`.scroll((detail_scroll, 0))`), and `adf_to_rich` (ADR 0010) already maps an ADF body to
  styled `RichLine`s. So comments can be **appended to the same scrollable paragraph** and
  become scrollable for free.

This is the fourth read-only slice of the active-collab-cli feature-parity program (Group A,
A4). AC-cli shows a comment thread in the task detail; Jira issues carry ADF comment bodies.

The one real UX fork — what `j`/`k` should do — was decided with the user: **plain vim scroll
aliases**, not per-comment focus/jump. Per-comment focus + scroll-into-view was considered and
explicitly deferred (it needs shared description/comment line-offset math between the pure model
and the view — the same off-screen-focus coupling A2 deferred for links).

## Decision

1. **Display comments read-only in `view_detail`**, appended after the description, inside the
   same scrollable `Paragraph`. Mirror the CLI layout for consistency (DRY the chrome):
   - When `issue.comments` is non-empty: a blank line, then a `{t("Comments")}:` header line,
     then per comment a `[{author}] {created}` header line (author → `t("Unknown")` fallback,
     `created` → empty-string fallback, exactly as `render_comment_human`) followed by the
     comment body rendered via `adf_to_rich` mapped to ratatui spans, then a trailing blank line.
   - When `issue.comments` is empty: render **no** comments section at all (mirrors the CLI,
     which prints nothing) — no header, no notice, no new i18n key.
   - Comment bodies are styled but **not** part of `detail_links`; they are mapped with no
     focused-link highlight (link nav stays description-only — extending Tab into comments is a
     later refinement, not this slice).
2. **Extract a pure helper** `detail_comment_lines(issue) -> Vec<Line<'static>>` that builds the
   comment section, so `view_detail` stays within the complexity ceiling (the debt just cleared
   in issue 0023 must not regress).
3. **`j`/`k` are global vim aliases** for `Msg::Down`/`Msg::Up` in `map_key_in_normal_mode`
   (`src/tui/shell.rs`). The key map stays screen-agnostic (`update` already dispatches Down/Up
   by screen: list-selection on List, `detail_scroll` on Detail), so `j`/`k` naturally scroll the
   detail (description + comments) and, as a vim-idiomatic bonus, move the list selection on List.
   No new `Msg`, no new `Cmd`, no new `Model` state.
4. **Footer discoverability:** update the detail footer hint to advertise `j/k` alongside `↑/↓`,
   via `t()` + a pt_BR catalog entry (the only new i18n key). `Comments`/`Unknown` already have
   pt_BR entries and are reused.

## Consequences

- **Positive:** comments become visible and scrollable in the TUI with a minimal, robust slice —
  no new model state, no layout-offset duplication, no client/data change. Reuses ADR 0010's
  `adf_to_rich` and the CLI's comment format and i18n keys. `Up/↓` and `j/k` scroll uniformly
  through description + comments.
- **Negative / deferred:** no per-comment focus/highlight and no jump-to-next-comment
  (scroll-into-view) — a long thread is reached by scrolling. Recorded as a future refinement
  (A5): per-comment focus needs a shared pure line-offset helper between model and view.
- **Deferred (unchanged):** comment link navigation via Tab (stays description-only); comment
  **writes** (create/edit/delete) remain parked behind the constitution's read-only boundary
  (Group C).

## Alternatives considered

- **Per-comment focus + `j`/`k` jump + scroll-into-view.** The richer parity behavior, but it
  couples the view's line layout into the pure model (to compute each comment's start line for
  `detail_scroll`), duplicating layout math that drifts when spacing changes — the same coupling
  A2 deferred for off-screen link focus. Deferred to A5; the user chose plain scroll for A4.
- **A separate comments pane / scroll region.** Rejected: a second scroll offset and layout split
  for no read-only benefit; the single scrollable paragraph already gives uniform navigation.
- **Reuse the CLI `render_comment_human` directly.** Rejected for the body: the CLI path is
  `adf_to_plain_text` (byte-stable plain text); the TUI wants `adf_to_rich` styling (ADR 0010).
  The header format (`[author] created`) and the `t("Comments")`/`t("Unknown")` keys are reused.
