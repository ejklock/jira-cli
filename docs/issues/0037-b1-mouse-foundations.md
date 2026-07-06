---
type: Issue
title: "B1 — mouse foundations: capture lifecycle, wheel navigation, card click drills in"
description: Enable mouse capture for the browse session (unconditional teardown); wheel maps to the existing Up/Down msgs on both screens; a left click on any row of a visible card selects and opens that issue via a pure hit-test reusing the view's own layout functions; clicks outside cards are no-ops; search mode swallows mouse input; no mouse event ever exits.
status: done
labels: [tui, input, mouse, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B1 — mouse foundations

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B1 per
[ADR 0017](/adr/0017-mouse-support-browse-tui.md), behaviors pinned by
[BDR 0009](/bdr/0009-browse-mouse-interactions.md) S1–S7.

Single layout source: the click resolver calls `list_layout_chunks` +
`first_visible_card` + `CARD_HEIGHT` — no duplicated geometry. Detail-screen
clicks (inline links, Ctrl/Cmd+click) belong to the R-B3 slice; app-managed
text selection to R-B2's.
