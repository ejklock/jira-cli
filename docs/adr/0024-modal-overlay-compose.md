---
type: ADR
title: "A reusable centered modal overlay hosts the comment compose (modal-first, no inline detour); server-truth refresh after every mutation"
description: Introduce src/tui/modal.rs — a pure centered-Rect layout (modal_area) plus a render helper (render_modal) that strongly dims the backdrop (DIM + dark background), Clears the box, and draws a rounded bordered panel with title/body/hint/status and optional buttons. The comment compose ('c' on the detail) is the first adapter; the C4 delete-confirm is the second (the seam's deletion test). While a modal is open its keymap owns all input. On a successful write the thread re-derives from the server via the existing refresh-detail path — never an optimistic local mutation.
status: Accepted
supersedes:
superseded_by:
tags: [tui, modal, overlay, comments, compose, write, consistency]
timestamp: 2026-07-07T00:00:00Z
---

# 0024. Modal overlay + comment compose

## Context

C1/C2 landed the write seam and its CLI consumer; the TUI cannot write yet
([ADR 0015](/adr/0015-comment-write-enablement.md) R-C1). The fork base
shipped compose **inline** in the scrollable detail (its ADR 0034), then
migrated to a centered modal after direct user feedback (its ADR 0039:
"melhor criarmos um componente reutilizável de modal", ~70% sizing, strong
dim). jira-cli adopts the **end state directly** — no inline detour to
re-learn the same lesson. The view layer already proves overlays work here:
the B3 selection highlight draws over the content after the detail render.

## Decision

1. **`src/tui/modal.rs`, flat.** The primitive lives beside `panel.rs` (this
   TUI has no `screens/`/`widgets/` split; creating a submodule for one
   member is premature — revisit if a third primitive appears).
2. **The primitive.** `modal_area(frame_area, desired_w, desired_h) -> Rect`
   is pure (centers, clamps with a margin, never overflows — headlessly
   unit-tested). `render_modal(frame, frame_area, ModalContent)` in order:
   (a) strongly dims every backdrop cell — `Modifier::DIM` **plus** a dark
   background from a new `theme.rs` style, so the thread reads as behind,
   not transparent; (b) sizes the box to ≈70% of the frame with a
   content-driven minimum, centers via `modal_area`, `Clear`s it; (c) draws
   a rounded bordered box (theme modal style, matching the panels) with
   title, body lines, and an in-box bottom hint/status line; (d) returns the
   modal `Rect` + any button spans so callers register click targets in
   modal-relative coordinates. `ModalContent`: title, body lines (rich run
   channel), optional hint, optional status, optional buttons (C4 uses
   buttons; compose does not).
3. **Compose is a mode, rendered through the modal.** `Model` gains
   `compose: Option<Compose>` (`buffer: String`,
   `status: Idle | Submitting | Error(String)`; an edit variant arrives in
   C4). **`c` on the detail** (issue loaded) opens it. Compose keymap:
   printable chars append; **Enter inserts a newline**; Backspace deletes;
   **Ctrl+S submits**; **Esc cancels** (discards the draft, stays on the
   detail). While a modal is open **its keymap owns all keys** and mouse
   events reach neither the detail machinery (links/selection/scroll) nor
   the list — no input leaks through the backdrop.
4. **Submit.** A non-empty (trimmed) buffer emits one
   `Cmd::SubmitComment { key, body }` and sets `Submitting` (the in-box
   status shows it; the TUI keeps redrawing — the write is a background
   effect). An empty buffer is a no-op. The shell spawn calls the C1
   `client.add_comment` and replies `Msg::CommentMutationOk` /
   `Msg::CommentMutationErr(reason)`.
5. **Server-truth refresh (port of fork ADR 0035).** `CommentMutationOk`
   clears the compose and returns exactly one refresh-detail Cmd for the
   open issue (the existing detail-load path with cache busting — reusing
   its single-flight discipline; the fresh payload replaces the thread).
   **No optimistic mutation anywhere**: the mutation path constructs no
   local comment (server owns id/author/timestamp/normalized body).
   `CommentMutationErr` keeps the buffer, sets `Error(localized reason)`
   (401 reuses the E2 re-auth message), and emits **no** refresh — lossless
   retry/abandon.
6. **Chrome.** The modal owns its hint (`Ctrl+S send · Esc cancel`) and
   transient status **inside the box**; the main footer keeps the browse
   hint. All new strings are English-source keys with pt-BR catalog entries
   (ADR 0006).

## Alternatives considered

- **Inline compose in the scrollable content** (fork's first version).
  Rejected — it scrolls with the thread, has no focus framing, and the fork
  user explicitly rejected it; adopting the corrected end state is the point
  of forking.
- **Optimistic local insert.** Rejected — fabricates server-owned fields
  (id, timestamp, author, ADF normalization); one round-trip after submit
  buys guaranteed consistency (fork ADR 0035's measured trade).
- **A modal stack / z-index manager.** Rejected (YAGNI) — one active modal
  derived from `compose` (later `confirm_delete`) covers every current need.

## Consequences

**Positive:** the TUI gains its first write with a focus-framed centered
overlay; the primitive is a real seam (C4's confirm is the second adapter —
its deletion test); the mutation arms stay tiny (set state + emit one
existing Cmd). **Trade-offs:** `view()` gains an overlay branch and a
per-frame backdrop dim pass (bounded by the viewport); a brief
"Submitting…" beat replaces instant insertion.

## Related

- ADR: [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md) (the add_comment seam), [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md) (§4 server-truth), [/adr/0014-tui-visual-design-system.md](/adr/0014-tui-visual-design-system.md) (theme home), [/adr/0019-app-managed-text-selection.md](/adr/0019-app-managed-text-selection.md) (overlay precedent)
- BDR: [/bdr/0015-comment-compose-behaviors.md](/bdr/0015-comment-compose-behaviors.md)
- Issues: [/issues/0047-c3a-modal-primitive.md](/issues/0047-c3a-modal-primitive.md), [/issues/0048-c3b-compose-post-refresh.md](/issues/0048-c3b-compose-post-refresh.md)
- Fork base: `active-collab-cli` ADR 0034/0035/0039, BDR 0024/0026
