---
type: Issue
title: "B3 — app-managed text selection in the detail (drag highlight + copy on release)"
description: Unmodified left drag on the detail body selects text in logical coordinates (pre-wrap line + char offset) resolved through compose_detail's extended metadata; release copies the chrome-free logical text via the existing Cmd::CopyToClipboard + 'Copied ✓' status; plain click clears; Ctrl/Super press keeps link activation and never selects.
status: done
labels: [tui, mouse, selection, clipboard, parity]
blocked_by: 0037, 0039
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B3 — app-managed text selection + clipboard

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B2 per
[ADR 0019](/adr/0019-app-managed-text-selection.md), behaviors
[BDR 0011](/bdr/0011-detail-text-selection-behaviors.md) S1–S10 —
including the three regression scenarios ported from the fork's shipped
extraction bugs (S5 chrome-free copy, S6 display-width column mapping,
S7 wrap-seam integrity).

Design: `compose_detail` metadata gains `(logical_line, char_start, char_len)`
provenance + chrome-free `logical_lines`; model gains
`selection: Option<{anchor, cursor, dragged}>` in logical coords; shell maps
Down/Drag/Up through pure view resolvers (`detail_pos_at`, `selection_text`);
highlight, hit-testing and extraction all read the one geometry pass.

Delivered with a shared `DetailGeometry` seam so `detail_link_at`,
`detail_pos_at` and the highlight can never drift, and with provenance
extended to all three panels (Details meta, Description, Comments) — BDR 0011
S5 covers any bordered panel.

**Known follow-ups (review observations):** (a) no test kills the
identity mutation on `normalize_selection` — add a backward-drag
(anchor > cursor) extraction test; (b) `selection_text` recomposes at
unbounded width, so an ellipsized Details meta value copies its full
un-truncated text while the highlight covers the truncated display —
practically benign, documented divergence.
