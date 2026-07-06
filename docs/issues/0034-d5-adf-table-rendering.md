---
type: Issue
title: "D5 — ADF table rendering in detail/comments"
description: rich_node handles table/tableRow/tableHeader/tableCell — one line per row, cells joined by ' │ ', header row bold. Closes the last rich-text coverage gap vs the fork base (strike/underline/codeBlock already render).
status: done
labels: [adf, richtext, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## D5 — ADF tables

Implements [ADR 0014](/adr/0014-tui-visual-design-system.md) §6; behavior
[BDR 0007](/bdr/0007-tui-visual-design-behaviors.md) S9.

### Scope

- `src/render.rs` `rich_node`: handle `table` → rows; `tableRow` → one
  `RichLine`, cells joined by ` │ `; `tableHeader` cells bold; nested block
  content inside a cell flattens to its text (legible, not spreadsheet).
- Renders identically in CLI `get` human output and TUI detail (single
  mapper — JSON/text no-drift holds).

### Acceptance

- BDR 0007 S9 passes; mapper unit tests: header+data table, empty cell,
  cell with marks (bold link), nested paragraph in cell.
- `agent_json` contract unchanged (raw ADF stays raw).
- Suite green; clippy `--all-targets -D warnings`, fmt, comment-policy clean.
