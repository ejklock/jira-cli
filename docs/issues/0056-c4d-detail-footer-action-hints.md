---
type: Issue
title: "C4d — detail footer action-key hints: advertise the comment-focus/edit/delete/reply and status keys in the browse-detail footer"
description: The browse-detail footer gains the comment and transition affordances. A new FooterMode::DetailComment (a comment is focused) shows '[ ] focus · e edit · d delete · r reply · s status · Esc/b back · q quit'; the base Detail and DetailLink footers gain '[ ] focus' and 's status'. The footer ADVERTISES the comment actions whenever a comment is focused; ownership stays enforced at invocation (the "not your comment" hint on e/d), so no per-ownership footer variant is needed. Pure footer_mode + footer_hint change (ADR 0014 §5) — one string per FooterMode.
status: done
labels: [tui, chrome, footer, comments, transition, parity]
blocked_by: 0053, 0055
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## C4d — detail footer action-key hints

Finishes the C4 chrome deferred across C4b/C4c and the transition slice: the
browse-detail footer now advertises the comment actions ([ ] focus, e edit,
d delete, r reply) and the status key (s). Refines [BDR 0017](/bdr/0017-comment-action-behaviors.md)
(footer) and [BDR 0018](/bdr/0018-status-transition-behaviors.md) (the `s` key);
no new ADR — it is finalized chrome for already-decided behavior.

Scope: `src/tui/model.rs`, `src/tui/view.rs`, `locales/pt_BR.json`,
`tests/unit/tui.rs`, `tests/unit/tui_render.rs`.

- **`FooterMode::DetailComment`** — added; `footer_mode` returns it when
  `detail_focused_comment.is_some()` (precedence over `DetailLink`).
- **Footer strings** (all via `t()`, keeping the established double-space
  separator — one style across every `FooterMode`, no per-family middot split):
  - `Detail`: `↑/↓ j/k scroll  [ ] focus  s status  Esc/b back  q quit`
  - `DetailLink`: `↑/↓ j/k scroll  Tab next link  Enter open  s status  Esc/b back  q quit`
  - `DetailComment`: `[ ] focus  e edit  d delete  r reply  s status  Esc/b back  q quit`
- **Advertise-then-enforce:** the footer shows `e`/`d`/`r` whenever a comment is
  focused; ownership is enforced when the action runs (the localized "not your
  comment" hint on `e`/`d`), so the footer needs no own/not-own split.
- Tests: `footer_mode` returns `DetailComment` when a comment is focused;
  the rendered detail footer contains the new affordances.

**Delivered 2026-07-16.** `FooterMode::DetailComment` added; `footer_mode`
returns it for `Screen::Detail` when `detail_focused_comment.is_some()`, with
precedence **before** the `detail_focused_link.is_some()` → `DetailLink` arm (a
focused comment wins over a focused link). `footer_hint` now returns, with the
established double-space separator — Detail: `↑/↓ j/k scroll  [ ] focus  s status
 Esc/b back  q quit`; DetailLink: `↑/↓ j/k scroll  Tab next link  Enter open  s
status  Esc/b back  q quit`; DetailComment: `[ ] focus  e edit  d delete  r
reply  s status  Esc/b back  q quit`. The three new/updated strings were added to
the pt_BR catalog (the two orphaned old Detail/DetailLink English keys removed).
`every_footer_mode_advertises_only_bound_keys` extended so every advertised key
(`[`, `]`, `e`, `d`, `r`, `s` — all already bound in `map_normal_char_key`) is
proven bound; render tests prove the focused-comment footer shows the comment
affordances and the base footer shows only `[ ]`/`s`. No keymap, `shell.rs`, or
`modal.rs` change. Reviewer: approved, 8/8 ACs, confidence 0.97.
