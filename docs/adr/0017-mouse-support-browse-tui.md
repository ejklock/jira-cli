---
type: ADR
title: Mouse support in the browse TUI (capture lifecycle, wheel, card click)
description: Enable crossterm mouse capture for the browse session; wheel scroll maps to the existing Up/Down msgs (screen-aware by construction); a left click on any row of a visible list card selects and opens that issue via a pure hit-test that reuses the view's own layout functions. No mouse event ever exits the app.
status: Accepted
supersedes:
superseded_by:
tags: [tui, input, mouse, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0017. Mouse support in the browse TUI

## Context

The browse TUI is keyboard-only; the fork base ships mouse interaction (wheel
scroll, click-to-drill on task cards — fork BDR 0001/0004/0020) and
[PRD 0003](/prd/0003-active-collab-parity.md) R-B1 ports it. The fork's very
first regression class — a mouse event quitting the whole app — is the
invariant to pin: **no mouse input ever exits.** The fork's later hit-test
refactor arc (its ADRs 0043–0045/0051) is deliberately not ported as
architecture; its end-state lesson is: **one layout source** — click resolution
must reuse the exact functions the view renders with, never duplicate geometry.

## Decision

1. **Capture lifecycle in the shell.** `EnableMouseCapture` is issued together
   with `EnterAlternateScreen`; teardown issues `DisableMouseCapture`
   unconditionally with `LeaveAlternateScreen` (safe regardless of state).
2. **Wheel = the existing navigation msgs.** `ScrollUp`/`ScrollDown` map to
   `Msg::Up`/`Msg::Down` in the pure mouse mapper. Screen-awareness comes free:
   `update_up/update_down` already move the list selection on `Screen::List`
   and scroll the detail on `Screen::Detail`. No new scroll state.
3. **Click drills in via a pure hit-test with one layout source.**
   `view::list_click_card(model, area, x, y) -> Option<usize>` computes the
   clicked card index by calling the same `list_layout_chunks` +
   `first_visible_card` + `CARD_HEIGHT` the renderer uses. Any row of a card is
   the click target (whole-card y-range, fork BDR 0020 S5); clicks outside the
   card area, past the last visible card, or on an empty list resolve to `None`
   (no clamping-to-last for clicks — opening an issue the user did not click is
   worse than doing nothing; deviation from fork BDR 0001's select-only clamp,
   which predates drill-in cards).
4. **New `Msg::CardClicked(usize)`.** The update arm clamps the index, sets
   `selected`, and then behaves exactly like `Msg::Select` (open detail +
   `Cmd::LoadDetail`). Detail-screen clicks are a no-op in this slice (inline
   link/Ctrl+click activation is R-B3's slice).
5. **Search mode swallows mouse input.** While the search prompt is active the
   mouse mapper emits nothing, mirroring the key-mapper split.
6. **Mouse msgs ride the key path.** Mouse-generated msgs are fed through the
   same apply path as key msgs, so the D4 transient-status clearing treats a
   click like any other user input.

## Alternatives considered

- **Click selects only; Enter opens** (fork BDR 0001's original). Rejected: the
  fork's own end state (BDR 0020) moved to click-drills-in on cards, and that
  is the parity target.
- **Clamp out-of-range clicks to the last card** (fork BDR 0001). Rejected for
  drill-in semantics: a click on empty space must not open anything.
- **A separate hit-test module with its own geometry** (the fork's interim
  state). Rejected: duplicated layout math drifts; the resolver calls the
  view's layout functions directly.
- **Toggle-based mouse capture for text selection** (fork ADR 0012). Not this
  slice: the fork superseded it with app-managed selection (its BDR 0015),
  which R-B2 ports later; capture stays always-on here.

## Consequences

**Positive:** wheel + click work everywhere the list/detail work today; the
no-exit invariant is property-tested; zero new geometry code paths.

**Accepted trade-offs:** always-on capture blocks the terminal's native
click-drag text selection until the app-managed selection slice lands
(known, sequenced in the parity program); detail-screen clicks do nothing yet.

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B1.
- BDR: [/bdr/0009-browse-mouse-interactions.md](/bdr/0009-browse-mouse-interactions.md)
- Fork base: BDR 0001/0004/0020 (behavior), ADR 0028/0051 (single-layout-source lesson).
- Issue: [/issues/0037-b1-mouse-foundations.md](/issues/0037-b1-mouse-foundations.md)
