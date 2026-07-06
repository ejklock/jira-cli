---
type: Issue
title: "D1 — theme.rs palette + identity header bar + themed footer"
description: Central truecolor theme module (sober cool-retro palette), a logged-in identity header line (email · instance, +N more), and the footer restyled through the theme. First visible slice of the ADR 0014 design system.
status: done
labels: [tui, design, theme, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## D1 — theme + identity header + themed footer

Implements [ADR 0014](/adr/0014-tui-visual-design-system.md) §1–§2; behavior
[BDR 0007](/bdr/0007-tui-visual-design-behaviors.md) S1 (+ footer restyle).

### Scope

- New `src/tui/theme.rs`: the ADR 0014 palette as named `Style` constructors
  (`header_bar`, `section_title`, `column_header`, `selected`, `badge`,
  `link`, `footer`, `due_style(bucket)`); no `Color::Rgb` outside it.
- `view()` gains a header region (header + content + footer); header text
  `"{email} · {instance}"` + `(+N more)`; model threads identity data in
  (shell reads config, view stays pure).
- Existing footer hints restyled via `theme::footer()`.

### Acceptance

- BDR 0007 S1 passes on `TestBackend` (single + multi instance).
- Footer renders in the theme footer style on list and detail.
- No inline `Color::Rgb` outside `theme.rs` (inspection + grep).
- Suite green; clippy `--all-targets -D warnings`, fmt, comment-policy clean.
