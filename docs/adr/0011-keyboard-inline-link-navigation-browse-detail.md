---
type: ADR
title: "Keyboard inline-link navigation in the browse TUI detail (Tab to cycle, Enter to open)"
description: Make inline body links in the browse TUI detail openable by keyboard — Tab cycles a focused link, Enter opens it (reusing the OpenUrl effect), the view highlights the focused link. Diverges deliberately from active-collab-cli's Ctrl/Cmd+click (mouse), because mouse support is a parked non-goal and PRD 0002's contract is keyboard. Builds on ADR 0010's adf_to_rich (the retained link href).
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, links, keyboard, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# 0011. Keyboard inline-link navigation in the browse TUI detail

## Context

[ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md) added styled ADF
rendering to the browse TUI detail, and each inline `link` mark now **retains its
`href`** in `RichStyle.link` (rendered underlined but not yet actionable). The
fork base `active-collab-cli` (AC) opens body links via **Ctrl/Cmd+click** — a
**mouse** interaction ([its ADR 0020/0025]). But `jira-cli`'s
[PRD 0002](/prd/0002-interactive-browse-tui.md) makes **keyboard the contract** and
lists **mouse support as a non-goal** (parked in the parity program's Group B).
So the parity feature ("open a link in the body") must be re-realized for the
keyboard, not copied from AC's mouse path.

Today the detail already has an `o` = "open link" affordance, but it opens the
**selected issue's** `/browse/KEY` URL (`update_open_link`, row-based) — not the
inline links inside the description body. There is no way to reach a URL embedded in
the text.

## Decision

Add **keyboard navigation of the description's inline links** in the Detail screen.

1. **Model** gains `detail_links: Vec<String>` (the description's inline link hrefs,
   in document order) and `detail_focused_link: Option<usize>` (which link is
   focused). Both are derived from the **same `adf_to_rich` model** the view renders,
   so link ordering is identical between the data and the display.
2. On `DetailLoaded`, populate `detail_links` (collect `RichSpan.style.link` in order)
   and set `detail_focused_link = Some(0)` if any links exist, else `None`. On `Back`
   (leaving the detail), clear both.
3. **`Tab`** → `Msg::FocusNextLink`: on the Detail screen with links present, advance
   the focused index (wrapping); else no-op.
4. **`Enter`** generalizes `Msg::Select` to "activate the current focus":
   - List screen → open the selected row's detail (unchanged).
   - Detail screen → if a link is focused, emit `Cmd::OpenUrl(href)`; else no-op.
5. **`o`** (`update_open_link`) is unchanged — it still opens the **issue's own** URL,
   available in both screens. `Tab`/`Enter` handle the **inline** links.
6. **View**: `view_detail` highlights the focused inline link (e.g. `REVERSED` on top
   of the existing `UNDERLINED`) by counting link spans in render order and comparing
   to `detail_focused_link`. The highlight appears only in Detail when a link is focused.
7. The `OpenUrl` effect and its opener are reused as-is (no new Cmd). Everything in
   `update` stays pure; the open is dispatched by the shell (Humble Object).

## Scope

- **Covers A2**: keyboard focus + open of the **description's** inline `link`-mark
  hrefs. It does **not** cover: `inlineCard`/smartlink nodes (a later refinement),
  auto-scrolling to an off-screen focused link (refinement), or links inside
  **comments** (comments aren't shown in the detail until A4 — link nav there rides on
  A4). Mouse activation stays parked (Group B).

## Alternatives considered

- **Copy AC's Ctrl/Cmd+click (mouse).** Rejected: mouse is a parked non-goal; the
  keyboard is PRD 0002's contract.
- **A numbered link picker** (press a digit to open the Nth link). Rejected for v1: a
  focus+Enter model is fewer concepts and matches the list's select idiom; a picker can
  come later if link counts get large.
- **A dedicated open key (e.g. `l`) instead of overloading Enter.** Rejected: `Enter`
  as "activate the focused thing" is the more intuitive generalization of the list's
  Enter=select, and keeps the key surface small (the shared key map is already near its
  complexity ceiling — no new production key beyond `Tab`).

## Consequences

**Positive:**

- Body links become reachable by keyboard — a real parity gain — reusing `adf_to_rich`
  (the href was already retained) and the existing `OpenUrl` effect; `update` stays pure.
- `Select` becomes a coherent "activate focus" across screens.

**Accepted trade-offs:**

- A focused link that is scrolled off-screen is still openable but not visible until
  scrolled to (auto-scroll deferred).
- `detail_links`/`detail_focused_link` add detail-screen state to the Model; cleared on
  `Back` to avoid leaking across issues.

## Related

- ADR: [/adr/0010-styled-adf-rendering-browse-tui-detail.md](/adr/0010-styled-adf-rendering-browse-tui-detail.md)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md)
- Issue: [/issues/0022-a2-keyboard-inline-link-navigation.md](/issues/0022-a2-keyboard-inline-link-navigation.md)
