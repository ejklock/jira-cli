---
type: Issue
title: "D4 — contextual footer + thin transient status line"
description: Mode-aware footer hints (list / search / detail / link-focus) and a thin one-line status row above the footer for transient feedback (copy confirmation, errors), auto-cleared on next input.
status: done
labels: [tui, design, footer, parity]
blocked_by: [0030]
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## D4 — contextual footer + status line

Implements [ADR 0014](/adr/0014-tui-visual-design-system.md) §5; behavior
[BDR 0007](/bdr/0007-tui-visual-design-behaviors.md) S7/S8. Port of the fork
base's N2 slice.

### Scope

- One pure `footer_hint(mode) -> String` source of truth over an explicit
  UI-mode enum (list, list+search, detail, detail+link-focus) — every hint a
  `t()` key; no hint advertises an unbound key (lesson 3345).
- `Model` gains a transient `status: Option<StatusMsg>` (info/error) rendered
  on a thin row above the footer; cleared by the next key event; copy-key
  feedback and fetch errors move onto it.

### Acceptance

- BDR 0007 S7, S8 pass on `TestBackend`.
- Every advertised key in every footer variant has a handler (test iterates
  variants × keymap).
- Suite green; clippy `--all-targets -D warnings`, fmt, comment-policy clean.
