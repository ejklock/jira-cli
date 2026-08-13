---
type: Issue
title: D1 — browse TUI opens an image attachment in the OS viewer
description: In the browse TUI, pressing the open key on a focused image/* attachment downloads it via the D2a seam to a temp file and launches the OS viewer (open/xdg-open/start); a non-image attachment keeps its open-URL-in-browser behavior.
status: done
timestamp: 2026-07-23T21:29:38Z
---

<!-- OKF frontmatter above carries the tracker metadata (`status`: open | in-progress |
     closed | superseded) that previously lived only in the directory index. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## D1 — browse TUI opens an image attachment in the OS viewer

Implements [ADR 0029](/adr/0029-attachments-authenticated-download-seam-download-attachments-and-external-image-viewer.md) §3
and [BDR 0020](/bdr/0020-attachment-download-and-external-image-viewer-behaviors.md) S8–S9.
Builds on the D2a seam ([issue 0060](/issues/0060-d2a-attachment-download-seam-download-attachment-on-jiraclient-with-same-origin-guard.md)).

### Scope

`src/tui/model.rs` (a `Msg::OpenAttachment`/`Cmd::OpenAttachment` pair + the pure
`update` mapping), `src/tui/shell.rs` (key→`Msg` mapping on a focused attachment; the
effectful `dispatch_cmd` arm that downloads to a temp file and shells the OS opener),
and TUI unit tests. KEPT: the existing non-image open-URL-in-browser path; the pure
`update`/model purity discipline (no I/O in `update`).

### Acceptance

- Focusing an `image/*` attachment and pressing the open key maps to
  `Cmd::OpenAttachment` (S8) — unit-tested on the key→`Msg`→`Cmd` mapping.
- Focusing a non-`image/*` attachment and pressing the open key keeps the existing
  open-URL behavior (S9) — unit-tested; the two paths are distinguished by mime type.
- `dispatch_cmd` for `OpenAttachment` downloads the bytes via the seam, writes a temp
  file (`tempfile`), and invokes the platform opener (`open`/`xdg-open`/`start`).
- The mime-routing decision is a pure, unit-tested function (no terminal needed).

### Plan

1. Add the `Msg`/`Cmd` variants; a pure helper `attachment_open_action(mime) ->
   {Viewer|Browser}` unit-tested for image vs non-image.
2. `update`: on the open key over a focused attachment, emit `OpenAttachment` (image)
   or the existing open-URL `Cmd` (non-image).
3. `dispatch_cmd`: async download → temp file → spawn OS opener; errors surfaced to
   the status line, never a panic.
4. TUI unit tests for the mapping; the opener shell-out is the thin I/O edge.
