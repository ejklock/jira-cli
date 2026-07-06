---
type: Issue
title: "B2b — Ctrl/Cmd+click opens the '[url]' token; plain click never navigates"
description: The B1 mouse mapper gains the click modifier set; on the detail screen a CONTROL/SUPER click resolves the span under the cursor via the pure, stateless view::detail_link_at (recomputes the renderer's own build/wrap/scroll/panel geometry) and emits Cmd::OpenUrl; wrapped fragments resolve the full href; plain clicks and non-link coordinates are no-ops.
status: open
labels: [tui, mouse, links, parity]
blocked_by: 0037, 0038
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B2b — modifier-gated click link activation

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B3 (activation half)
per [ADR 0018](/adr/0018-inline-body-links-modifier-click.md), behaviors
[BDR 0010](/bdr/0010-inline-body-link-behaviors.md) S5–S8.
