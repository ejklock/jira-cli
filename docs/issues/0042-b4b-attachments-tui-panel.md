---
type: Issue
title: "B4b — Attachments panel in the browse detail (inline, link rows, footnote)"
description: An Attachments (N) panel after Comments inside compose_detail's single pass — '[n] ↗ filename' link-styled rows whose cells carry the content URL as href (B2b click + B3 selection work with zero new machinery), blank-row breathing room, italic Ctrl/Cmd+click footnote, joins the global scroll (nothing clipped), empty list renders no panel.
status: done
labels: [tui, detail, attachments, parity]
blocked_by: 0041
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B4b — attachments TUI panel

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B4 (TUI half) per
[ADR 0020](/adr/0020-issue-attachments-detail-panel.md), behaviors
[BDR 0012](/bdr/0012-attachments-behaviors.md) S3–S8.

Delivered with zero new click/selection machinery — the rows' compose cells
carry `href`, so B2b activation and B3 selection just work. Footnote styled
with Modifier-only flags (italic+dim; no color, so no theme.rs constructor —
the Color::Rgb discipline targets colors). S8's scroll bound proven by a
scrollbar-appearance differential (outer frame borders make naive line counts
meaningless).
