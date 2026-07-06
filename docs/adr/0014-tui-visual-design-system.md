---
type: ADR
title: "TUI visual design system — port the vibrant-dashboard look from active-collab-cli"
description: Adopt the fork base's sober cool-retro truecolor design system — central theme.rs palette, identity header bar, per-issue list cards with colored relative due date, detail as stacked rounded panels in one global scroll with scrollbar, contextual footer + thin status line — adapted to Jira (project/due from the issue payload, no user_map fetch).
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, design, theme, ratatui, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0014. TUI visual design system (vibrant-dashboard parity)

## Context

The browse TUI is functionally complete (B0–B4, P1–P3, A1–A4) but visually
bare: the list is a flat `Table` (KEY/TYPE/STATUS/ASSIGNEE/SUMMARY) with a
`REVERSED` selection, the detail is flat labelled lines, styles are inline
modifiers with no palette, there is no identity context, no scrollbar, no due
date on the list ( `IssueRow` does not even carry it). The fork base solved
exactly this in its ADR 0009 (vibrant dashboard) + ADR 0026 (task cards),
user-tested through several iterations. [PRD 0003](/prd/0003-active-collab-parity.md)
R-D1..R-D5 mandates porting that end state.

## Decision

Port the **end state** of the AC design system, adapted to Jira:

1. **Palette — `src/tui/theme.rs`** (new module). The AC sober cool-retro
   truecolor set, verbatim: header bar `fg Rgb(102,204,204) bg Rgb(38,52,74) BOLD`;
   block/section titles `fg Rgb(102,204,204) BOLD`; table/column header
   `fg Rgb(140,165,196) BOLD`; selected row/card `fg Rgb(13,13,13) bg Rgb(210,160,90) BOLD`;
   count/badge amber `fg Rgb(210,160,90) BOLD`; link affordance
   `fg Rgb(120,190,130) UNDERLINED`; footer `fg Rgb(208,216,224) bg Rgb(38,52,74) BOLD`;
   due-date colors: overdue `Rgb(224,108,108)`, near (≤2 days) amber
   `Rgb(210,160,90)`, else default. All style construction goes through
   `theme.rs` — no inline `Color::Rgb` outside it.
2. **Identity header bar.** A one-line header above the content:
   `"{email} · {instance_name}"` from the `Instance` config, with `(+N more)`
   when issues aggregate multiple instances. **No network call** — unlike AC
   (which resolved a display name via `user_map`), Basic auth config already
   holds the identity; a display-name fetch is rejected as a new round-trip.
3. **List as per-issue cards.** Replace the `Table` with one bordered rounded
   card per issue: line 1 `KEY summary` (key styled as badge), line 2
   `{relative_due colored} · {status} · {project}`. `IssueRow` gains
   `duedate: Option<String>` and `project: Option<String>` threaded from the
   search payload (serde default — cached snapshots stay compatible; no extra
   fetch). Due text reuses the pure `relative_due` formatter (ADR 0013);
   color via `theme::due_style(bucket)`. Selection highlights the whole card.
   Status stays on the card (unlike AC, which pre-filtered to open tasks,
   jira lists arbitrary JQL results).
4. **Detail as stacked rounded panels, one global scroll.** A pure
   `panel_box(label, inner_lines, width)` primitive (rounded `╭╮╰╯─│`, label
   in the top border, body padded and fit by unicode display width) composes:
   a **Details** panel (2-column meta table: Title, Key, Status, Type,
   Assignee, Due), a **Description** panel, a **Comments (N)** panel with
   nested per-comment cards. The issue summary is promoted to the detail
   frame border title (truncated display-width-aware; falls back to the key).
   One `offset` over pre-built lines; the effective offset clamps to
   `lines.len() - viewport` at render time; a ratatui `Scrollbar` renders
   when content exceeds the viewport. Styled ADF spans (A1) are preserved
   inside the panels — the panel body is styled lines, not plain strings.
5. **Contextual footer + thin status line.** The footer hint is mode-aware
   (list / list+search / detail / detail+link-focus); transient messages
   (copy feedback, errors, later compose status) move to a thin one-line
   status row above the footer, auto-cleared on next input.
6. **ADF `table` rendering.** `rich_node` gains `table`/`tableRow`/
   `tableHeader`/`tableCell`: each row renders as one line, cells joined by
   ` │ `, header row bold — the AC R4 "legible, not spreadsheet" contract.
   (strike, underline, codeBlock already render — verified in the mapper.)

Delivered as five vertical slices: D1 palette+header ([0030](/issues/0030-d1-theme-header-footer.md)),
D2 list cards ([0031](/issues/0031-d2-list-cards-due.md)), D3 detail panels
([0032](/issues/0032-d3-detail-panels-scrollbar.md)), D4 footer/status
([0033](/issues/0033-d4-contextual-footer-status-line.md)), D5 ADF tables
([0034](/issues/0034-d5-adf-table-rendering.md)).

## Alternatives considered

- **Re-run the AC design exploration** (refined vs minimal vs vibrant).
  Rejected: AC already user-tested the alternatives and converged after
  real iterations (synthwave → sober); parity means adopting the outcome.
- **Named ANSI palette** for broader terminal support. Rejected — same
  trade-off AC accepted: target terminals are truecolor.
- **Fetch display name for the header** (`/rest/api/3/myself`). Rejected:
  new round-trip for cosmetic data; email+instance already identify.
- **Keep the list as a Table, add a DUE column.** Rejected: cards carry the
  two-line density the operator asked for in AC (D2) and unify list/detail
  visual language.

## Consequences

**Positive:** consistent visual system, identity + scroll affordance, urgency
visible on the list, one styling seam (`theme.rs`) instead of scattered
modifiers. **Trade-offs:** `view()` gains a header region; card lists render
fewer rows per screen; `IssueRow` grows two optional fields; panel/wrap
helpers must be display-width-correct (port the AC `fit_to_display_width`
approach with `unicode-width`). The TEA core stays pure — all new layout is
pure line-building, unit-tested with `TestBackend`.

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md),
  [/adr/0010-styled-adf-rendering-browse-tui-detail.md](/adr/0010-styled-adf-rendering-browse-tui-detail.md),
  [/adr/0013-relative-due-date-rendering.md](/adr/0013-relative-due-date-rendering.md)
- BDR: [/bdr/0007-tui-visual-design-behaviors.md](/bdr/0007-tui-visual-design-behaviors.md)
- Fork base: `active-collab-cli` ADR 0009 / ADR 0026 / BDR 0020 / issue D2, N2
