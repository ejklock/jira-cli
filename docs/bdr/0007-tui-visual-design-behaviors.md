---
type: BDR
title: "browse TUI visual design system — observable behaviors"
description: Given/When/Then scenarios for the vibrant-dashboard port — identity header, per-issue cards with colored relative due, stacked detail panels + scrollbar, contextual footer + thin status line, ADF table rendering.
status: Accepted
supersedes:
superseded_by:
tags: [tui, design, cards, panels, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0007. Browse TUI visual design system — behaviors

Pins the observable behavior of [ADR 0014](/adr/0014-tui-visual-design-system.md)
(PRD 0003 R-D1..R-D5). All scenarios render `view(model)` to a ratatui
`TestBackend` buffer with a fixed `today` injected — deterministic, no clock,
no network.

## Scenarios

**S1 — identity header.** Given a configured instance `acme` with email
`me@x.com`, when any browse screen renders, then the top line shows
`me@x.com · acme` in the header style; given issues from 2 instances, then the
suffix `(+1 more)` appears.

**S2 — issue card.** Given a list issue `PROJ-1 "Fix login"` due tomorrow,
status `In Progress`, project `Proj`, when the list renders, then it renders as
a bordered card: line 1 `PROJ-1 Fix login`, line 2 `tomorrow · In Progress · Proj`
with the due segment in the near-amber style.

**S3 — due colors.** Given issues due yesterday / today / in 5 days / no date,
then their due segments render overdue-red `overdue by 1 day`, amber `today`,
default `in 5 days`, and default `no due date` respectively (pt-BR via `t()`).

**S4 — whole-card selection.** Given the second issue selected, when the list
renders, then every cell of that card (borders and both lines) carries the
selected style, and no other card does.

**S5 — detail panels.** Given an issue with description and 2 comments, when
detail renders, then content is three rounded panels titled `Details`,
`Description`, `Comments (2)` in one scroll; the Details panel shows the
2-column meta rows (Title, Key, Status, Type, Assignee, Due); the frame border
title is the issue summary (ellipsized when longer than the width).

**S6 — scroll clamp + scrollbar.** Given detail content taller than the
viewport, then a scrollbar renders and scrolling past the end clamps so the
last content line stays at the bottom (no blank overscroll).

**S7 — contextual footer.** Given list mode, then the footer shows the list
hints; given search input active, the search hints; given detail with a
focused link, the link hints. Only one footer line at a time.

**S8 — thin status line.** Given a copy-key action, then a transient
confirmation renders on the status row above the footer and disappears on the
next key event; errors render in the error style on the same row.

**S9 — ADF table.** Given a description with an ADF `table` (header row +
2 data rows), when detail renders, then each row is one line with cells joined
by ` │ ` and the header row bold — no dropped cell text.

## Test design

One `TestBackend` render test per scenario (unit, pure); card/panel line
builders additionally unit-tested as pure functions (geometry: exact width,
CJK/wide glyphs, ellipsis). Mutation-sensitive assertions: full-line string
equality for S2/S3/S9, style-at-cell probes for S1/S4/S6. The slice issues
(0030–0034) link back to these scenarios instead of copying them.

## Related

- ADR: [/adr/0014-tui-visual-design-system.md](/adr/0014-tui-visual-design-system.md)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md)
- BDR: [/bdr/0006-browse-tui-interactions.md](/bdr/0006-browse-tui-interactions.md)
