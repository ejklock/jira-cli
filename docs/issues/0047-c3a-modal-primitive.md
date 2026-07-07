---
type: Issue
title: "C3a — reusable modal overlay primitive (modal_area + render_modal, dimmed backdrop, themed box)"
description: New src/tui/modal.rs with the pure centered/clamped modal_area and render_modal (strong DIM+dark-bg backdrop, Clear, ~70% rounded bordered box with title/body/hint/status/buttons, returns Rect + button spans); theme.rs gains the modal styles; headless layout tests + TestBackend render tests. No consumer yet — C3b (compose) is the first adapter, C4 (confirm) the second.
status: todo
labels: [tui, modal, widget, parity]
blocked_by:
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## C3a — modal primitive

Implements the primitive half of
[ADR 0024](/adr/0024-modal-overlay-compose.md) (BDR 0015 S5's layout/render
contract). Scope: `src/tui/modal.rs` (new), `src/tui/mod.rs` (barrel),
`src/tui/theme.rs` (modal styles), `tests/unit/tui_render.rs`.
