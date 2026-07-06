---
type: Issue
title: "D3 — detail as stacked rounded panels + title border + clamped scrollbar"
description: Detail content becomes stacked rounded panels (Details meta table with Title row, Description, Comments with nested cards) in one global scroll; issue summary promoted to the frame border title; ratatui Scrollbar with end-clamped offset. Styled ADF spans preserved inside panels.
status: done
labels: [tui, design, detail, parity]
blocked_by: [0030]
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## D3 — detail stacked panels + scrollbar

Implements [ADR 0014](/adr/0014-tui-visual-design-system.md) §4; behavior
[BDR 0007](/bdr/0007-tui-visual-design-behaviors.md) S5/S6.

### Scope

- Pure `panel_box(label, styled_lines, width)` primitive: rounded borders,
  label in top border, interior padding, every line fit to exact display
  width (`unicode-width`), styled spans preserved.
- Detail composes Details (2-col meta: Title, Key, Status, Type, Assignee,
  Due) + Description + `Comments (N)` panels into the single scroll buffer;
  summary → frame border title (ellipsized, display-width-aware).
- Offset clamps to `lines - viewport` at render; `Scrollbar` shown when
  content overflows; link focus (A2) and comment scroll (A4) keep working
  over the new line geometry.

### Acceptance

- BDR 0007 S5, S6 pass on `TestBackend`; panel_box geometry unit tests
  (exact width incl. wide/CJK glyphs, padding, empty body).
- A2 link Tab-cycle and A4 comment scroll regressions stay green.
- Suite green; clippy `--all-targets -D warnings`, fmt, comment-policy clean.
