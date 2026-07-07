---
type: BDR
title: Detail text-selection behaviors (drag, highlight, copy, clear)
description: Observable behaviors for app-managed text selection in the browse TUI detail — drag highlights, release copies logical text via the existing clipboard seam, plain click clears, modifier press never selects — including the three regression scenarios ported from the fork's shipped extraction bugs (chrome-free copy, display-width column mapping, wrap-seam integrity) and scroll stability.
status: Accepted
supersedes:
superseded_by:
tags: [tui, mouse, selection, clipboard, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0011. Detail text-selection behaviors

Behaviors for [ADR 0019](/adr/0019-app-managed-text-selection.md), ported from
the fork base's BDR 0015 (S1–S10) and reconciled with the
[BDR 0010](/bdr/0010-inline-body-link-behaviors.md) pointer model.

## Scenarios

### S1 — drag highlights

- **Given** the detail screen shows an issue body
- **When** the user presses the left button on a body cell and drags to
  another cell
- **Then** the covered span renders with the selection-highlight style,
  updating as the drag moves; anchor→cursor is normalized to reading order
  (dragging backwards or upwards selects the same span as forwards).

### S2 — release copies

- **Given** an active drag selection
- **When** the user releases the button
- **Then** the selected text is copied via the existing clipboard command,
  the status line shows the existing `Copied ✓` feedback, and the highlight
  stays visible until the next click.

### S3 — plain click clears and never navigates

- **Given** the detail screen (with or without an existing selection)
- **When** the user presses and releases without dragging and without a
  modifier
- **Then** any selection is cleared, nothing is copied, and no URL opens —
  the plain-click-never-navigates invariant (BDR 0010 S6) holds.

### S4 — modifier press never selects

- **Given** the detail screen
- **When** the user presses with Ctrl/Super held
- **Then** no selection starts; link activation (BDR 0010 S5) behaves exactly
  as before.

### S5 — copy is logical text, chrome-free *(fork bug regression)*

- **Given** a selection spanning a full visual row inside a bordered panel
- **When** the copy happens
- **Then** the clipboard text contains only the content characters — never
  box-drawing borders, panel padding, or the scrollbar column.

### S6 — display-width column mapping *(fork bug regression)*

- **Given** a line starting with a double-width character (e.g. an emoji)
- **When** the user selects starting just after it
- **Then** the copied text starts at the correct character — a display column
  is never treated as a char index, and no character is eaten or duplicated.

### S7 — wrap-seam integrity *(fork bug regression)*

- **Given** a logical line wrapped across two visual rows
- **When** the user selects across the wrap seam
- **Then** the copied text is the contiguous logical text with no character
  dropped or repeated at the seam, and highlight covers both fragments.

### S8 — scroll-stable selection

- **Given** an active selection
- **When** the user scrolls the detail (wheel or keys)
- **Then** the selection still covers the same logical text; the highlight
  follows the content, and a subsequent copy yields the same text.

### S9 — clipboard degradation

- **Given** no clipboard tool is available on the host
- **When** a copy is attempted
- **Then** the app does not crash or corrupt the terminal; the failure is
  silent-best-effort, matching the existing copy-key contract.

### S10 — out-of-content coordinates clamp

- **Given** an active drag
- **When** the pointer moves past the end of a line or outside the content
  area
- **Then** the cursor clamps to the nearest valid logical position; the drag
  never panics or selects chrome.

## Related

- ADR: [/adr/0019-app-managed-text-selection.md](/adr/0019-app-managed-text-selection.md)
- BDR: [/bdr/0010-inline-body-link-behaviors.md](/bdr/0010-inline-body-link-behaviors.md), [/bdr/0009-browse-mouse-interactions.md](/bdr/0009-browse-mouse-interactions.md)
- Issue: [/issues/0040-b3-app-managed-selection.md](/issues/0040-b3-app-managed-selection.md)
- Fork base: BDR 0015 (S1–S10, incl. the three shipped-bug scenarios S8/S9/S10 there).
