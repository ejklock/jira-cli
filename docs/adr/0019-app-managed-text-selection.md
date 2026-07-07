---
type: ADR
title: App-managed text selection in the detail (logical-coordinate drag + clipboard)
description: Mouse press-drag-release selects detail text with a drawn highlight and copies it on release via the existing clipboard seam. Selection lives in LOGICAL coordinates (pre-wrap line + char offset) resolved through compose_detail's single geometry pass — killing by construction the fork's three reported extraction bugs (display-column-as-char-index, eaten chrome origin, wrap-seam loss). Plain click clears; modifier press never selects.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, mouse, selection, clipboard, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0019. App-managed text selection with logical coordinates

## Context

B1 keeps mouse capture always-on, which blocks the terminal's native
click-drag selection — the known trade-off sequenced for this slice.
The fork base solved it with app-managed selection (its BDR 0015 superseding
the capture-toggle BDR 0006; ADR 0021): the app draws the highlight and copies
on release. Its BDR records **three shipped extraction bugs**: box-drawing
chrome copied along, a leading double-width emoji eating the next letter
(display columns misread as char indices), and a meta value copied with its
first character eaten (chrome origin subtracted differently by highlight vs
extraction). [PRD 0003](/prd/0003-active-collab-parity.md) R-B2 ports the end
state; the bugs inform the design.

## Decision

1. **Selection state is LOGICAL, not visual.** `compose_detail` (the B2b
   single geometry pass) is extended so per-visual-row cell metadata also
   records `(logical_line, char_start, char_len)` — char indices into the
   pre-wrap logical line — and the compose result carries the chrome-free
   `logical_lines: Vec<String>`. The model stores
   `selection: Option<{ anchor: (line, ch), cursor: (line, ch), dragged }>`.
   Consequences by construction: extraction slices logical text (never the
   rendered frame → no chrome, no wrap seams — fork S8/S9); scrolling never
   changes the selected span (fork S10); highlight and extraction share one
   origin (they read the same metadata — the fork's divergence bug cannot
   exist).
2. **Column→char resolution walks display widths.** A pure
   `view::detail_pos_at(model, area, x, y)` maps a viewport cell to a logical
   position by walking the row's cells and then the cell's chars by
   unicode display width (the same math panel.rs uses) — a display column is
   never treated as a char index (fork's emoji bug). Coordinates past the line
   end clamp to the line end; rows outside the content clamp for drags.
3. **Pointer model (reconciled with ADR 0018's modifier gate).** On the detail
   body: unmodified left **down** anchors a selection (replacing any previous
   one); **drag** moves the cursor (anchor→cursor normalized to reading
   order); **release after a drag** extracts the logical span and emits the
   existing `Cmd::CopyToClipboard` + the existing `Copied ✓` status, keeping
   the highlight until the next click; **release without drag** (plain click)
   just clears the selection and never navigates. A **Ctrl/Super press never
   starts a selection** — activation (B2b) and selection stay disjoint. List
   screen and search mode keep their B1 semantics untouched.
4. **Shell resolves, model decides.** The shell maps Down/Drag/Up to
   `Msg::SelStart/SelDrag((line, ch))/SelEnd(Option<String>)` using the pure
   view resolvers (extraction happens at release via `view::selection_text`);
   the model stays free of terminal types and owns the state transitions.
5. **Highlight** renders the covered cells with a theme selection-highlight
   style (REVERSED-class, from a theme.rs constructor).
6. **Clipboard degradation** keeps the existing copy-key contract: the shell
   effect is best-effort (pbcopy/xclip/wl-copy), never panics; the status
   feedback comes from the pure update.

## Alternatives considered

- **Capture-toggle selection mode (fork ADR 0012/BDR 0006).** Rejected — the
  fork itself superseded it: the app can draw no feedback and the terminal
  grabs panel borders.
- **Visual/absolute-cell selection state (the fork's interim).** Rejected: it
  is the direct source of all three recorded bugs and breaks under scroll.
- **A clipboard crate (arboard).** Rejected: the repo already ships a working
  external-tool clipboard seam used by the copy-key feature; one seam.

## Consequences

**Positive:** click-drag selection works with visible feedback and correct
text; the three fork bug classes are structurally impossible; zero new
geometry sources; reuses the existing clipboard + status seams.

**Accepted trade-offs:** selection is per-logical-span within the detail
content only (no cross-panel rectangular selection); plain click on the detail
body now clears a selection (previously a pure no-op).

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B2.
- BDR: [/bdr/0011-detail-text-selection-behaviors.md](/bdr/0011-detail-text-selection-behaviors.md)
- ADR: [/adr/0017-mouse-support-browse-tui.md](/adr/0017-mouse-support-browse-tui.md), [/adr/0018-inline-body-links-modifier-click.md](/adr/0018-inline-body-links-modifier-click.md)
- Fork base: BDR 0015 (supersedes its 0006), ADR 0021, ADR 0050.
- Issue: [/issues/0040-b3-app-managed-selection.md](/issues/0040-b3-app-managed-selection.md)
