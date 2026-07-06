---
type: BDR
title: "Browse TUI mouse: wheel navigates, card click drills in, nothing exits"
description: Observable mouse behavior of the browse TUI — wheel maps to the existing up/down navigation on both screens, a left click on any row of a visible card selects and opens that issue, clicks outside cards are no-ops, search mode swallows mouse input, and no mouse event ever exits the app. Includes the Test Design matrix.
status: Accepted
supersedes:
superseded_by:
tags: [tui, input, mouse, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0009. Browse TUI mouse interactions

Realizes [ADR 0017](/adr/0017-mouse-support-browse-tui.md) (port of fork-base
BDR 0001/0004/0020 mouse behavior, adapted to drill-in cards).

## Scenarios

### S1 — Wheel navigates the list
**Given** the list screen, **When** the operator scrolls the wheel up/down,
**Then** the selection moves exactly as `Up`/`Down` keys do (clamped at both
ends, windowing follows).

### S2 — Wheel scrolls the detail
**Given** the detail screen, **When** the wheel scrolls,
**Then** the detail content scrolls exactly as the `↑/↓`/`j/k` keys do
(clamped by the existing scroll clamp).

### S3 — Click on any card row drills in
**Given** visible cards, **When** the operator left-clicks any of the rows of
the card for issue *k* (whole-card y-range is one target),
**Then** *k* becomes selected and its detail opens (same contract as `Enter`:
cache-or-fetch via `Cmd::LoadDetail`).

### S4 — Click outside any card is a no-op
**Given** the list screen, **When** the click lands on the header, footer,
status row, or below the last visible card (or the list is empty),
**Then** nothing changes — no selection jump, no open, no panic.

### S5 — Windowed click resolves the on-screen card
**Given** a selection deep in the list (window scrolled),
**When** the operator clicks the first visible card,
**Then** the resolved index is the windowed one (`first_visible_card` offset),
not index 0.

### S6 — No mouse event ever exits
**Given** any screen and any model state, **When** any mouse event arrives
(click anywhere, wheel past the edges, drag, middle/right buttons),
**Then** the app never requests exit — only `q`/`Esc` semantics exit.

### S7 — Search mode swallows the mouse
**Given** the search prompt is active, **When** any mouse event arrives,
**Then** no message is produced (typing/navigation state untouched).

## Test Design

| Scenario | Level | Technique | Instrument / assertion |
|---|---|---|---|
| S1 | unit | example | mouse mapper: ScrollUp→`Msg::Up`, ScrollDown→`Msg::Down` (normal mode); update_up/down list arms already pinned |
| S2 | unit | example | same mapper output on detail screen; existing detail-scroll update tests cover the branch |
| S3 | unit | example | `list_click_card` resolves each of a card's rows to its index; `update(CardClicked(i))` sets selected=i, pushes detail, emits `Cmd::LoadDetail` (mirror of the `Select` tests) |
| S4 | unit | boundary | y on header/footer/past-last-card → `None`; empty list → `None`; `CardClicked` beyond len is clamped-safe (no panic, no open of a phantom row) |
| S5 | unit | example | seeded window (selected deep) — click y of the first visible slot resolves to `first_visible_card` value, not 0 |
| S6 | unit | property/invariant | for every `MouseEventKind` over both screens, mapper output never includes a quit; `should_quit` stays false |
| S7 | unit | example | mapper with search_active=true returns `None` for click and wheel |

The mapper and resolver are pure (no terminal); `list_click_card` calls the
same `list_layout_chunks`/`first_visible_card`/`CARD_HEIGHT` the renderer uses,
so the render windowing tests double as geometry oracles. The
`Enable/DisableMouseCapture` execution is the untestable shell seam, kept
minimal (inspection).

## References

- ADR: [/adr/0017-mouse-support-browse-tui.md](/adr/0017-mouse-support-browse-tui.md)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B1.
- Issue: [/issues/0037-b1-mouse-foundations.md](/issues/0037-b1-mouse-foundations.md)
