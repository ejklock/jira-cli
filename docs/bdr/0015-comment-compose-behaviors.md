---
type: BDR
title: "Comment compose: 'c' opens a centered modal over a dimmed thread; multi-line typing; Ctrl+S posts and the thread reloads from the server; failures keep the draft"
description: On the browse detail, pressing c opens a centered compose modal (dimmed backdrop, rounded box, in-box hint) — typing builds a multi-line buffer (Enter = newline), Esc discards, Ctrl+S posts through the C1 seam showing Submitting; on 2xx the modal closes and the thread re-renders from a fresh server fetch (never an optimistic insert); on failure the draft is preserved with a localized in-box error (401 = re-auth message) and no refresh. While the modal is open no key/mouse input reaches the detail or list machinery.
status: Accepted
superseded_by:
supersedes:
tags: [tui, comments, write, compose, modal, mutation]
timestamp: 2026-07-07T00:00:00Z
---

# 0015. Comment compose behaviors

## Context

First TUI write ([ADR 0024](/adr/0024-modal-overlay-compose.md), consuming
the [ADR 0022](/adr/0022-comment-write-seam.md) seam). Jira adaptation of the
fork base's BDR 0024 (S1–S3, S7) + BDR 0026 (modal rendering), modal-first.

## Textual Description

On the **browse detail** of a loaded issue:

- Pressing **`c`** opens a **centered compose modal**: the thread behind is
  strongly dimmed (still faintly visible), the box is rounded-bordered with
  the localized title, and an in-box bottom line shows
  `Ctrl+S send · Esc cancel`.
- Typing appends; **Enter inserts a newline** (never submits); Backspace
  deletes. The buffer renders inside the box as it grows.
- **Esc** closes the modal and discards the draft — the detail is unchanged,
  no write happens.
- **Ctrl+S** on a non-empty buffer posts the comment (C1 seam, ADF body);
  the in-box status shows **Submitting…** and the app keeps redrawing. An
  empty buffer does not submit.
- On **success** the modal closes and the comment thread **reloads from the
  server** (cache-busting refresh) — the new comment appears with its real
  server data. No locally-fabricated comment ever renders.
- On **failure** the modal stays open with the typed text **preserved** and
  a localized in-box error; a 401 shows the standard re-auth message. No
  refresh happens on failure.
- While the modal is open, **no input leaks**: list/detail keys (j/k, Tab,
  q, p…) and mouse events (wheel, click, drag/selection, link activation)
  do not reach the machinery behind the backdrop.

## Scenarios

**Scenario 1: open and type multi-line** — Given the detail view, When the
user presses `c`, types "Linha 1", presses Enter, types "Linha 2", Then the
modal shows a two-line buffer (the Enter became a newline, not a submit).

**Scenario 2: submit posts and the thread reloads** — Given a non-empty
buffer, When the user presses Ctrl+S, Then exactly one submit Cmd is emitted
for the open issue with the buffer as body, the status shows Submitting, and
on the write's success the model emits exactly one cache-busting detail
refresh Cmd and the compose clears.

**Scenario 3: cancel discards** — Given an open compose with typed text,
When the user presses Esc, Then the modal closes, the draft is gone, no Cmd
is emitted, and the detail is unchanged.

**Scenario 4: failure keeps the draft** — Given a submit whose write fails,
When the failure lands, Then the buffer is preserved, a localized error
shows in the box (401 → the re-auth message), and no refresh Cmd is emitted.

**Scenario 5: modal renders centered over a dimmed thread** — Given an open
compose, When the frame renders, Then backdrop cells carry DIM + the dark
backdrop background, the box is centered at ≈70% of the frame (clamped on
small terminals), and the title/hint render inside the box.

**Scenario 6: no input leakage** — Given an open compose, When the user
presses detail/list keys (j, k, q, p, Tab) or clicks/scrolls the backdrop,
Then the detail scroll, selection, links, and list state are unchanged (only
the compose buffer/status react); `q` does not quit while composing.

**Scenario 7: empty buffer never submits** — Given an open compose with an
empty (or whitespace-only) buffer, When the user presses Ctrl+S, Then no Cmd
is emitted and the modal stays open.

**Scenario 8: 'c' only acts on the detail** — Given the list or Projects
screen, When the user presses `c`, Then no compose opens (the key keeps its
existing meaning or is a no-op there).

## Test Design

Pure `update()` drives S1–S4/S6–S8 headlessly (compose state machine, Cmd
emission, leakage guards); `TestBackend` buffers prove S5 (cell-derived DIM
+ centered box + title/hint text); `modal_area` is unit-tested pure (center,
clamp, never overflow). The shell spawn is covered by a wiremock write test
(reply mapping Ok/Err), mirroring the existing spawn tests.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Multi-line buffer | unit (update) | 1 | after `c` + chars + Enter + chars: buffer contains `\n`; no Cmd | Enter = newline |
| Submit emits write | unit (update) | 2, 7 | Ctrl+S non-empty -> exactly one SubmitComment{key, body} + Submitting; empty -> no Cmd | submit gate |
| Ok -> one refresh | unit (update) | 2 | CommentMutationOk -> compose None + exactly one refresh-detail Cmd (cache-busting) for the open key | server-truth refresh |
| Err -> lossless | unit (update) | 4 | CommentMutationErr -> buffer intact, Error(status) set, zero refresh Cmds | draft preserved |
| Cancel | unit (update) | 3 | Esc -> compose None, no Cmd, detail state untouched | safe abandon |
| No key leakage | unit (update) | 6 | with compose open: j/k/q/p/Tab leave scroll/screen/selection unchanged | modal owns keys |
| No mouse leakage | unit (update) | 6 | wheel/click/drag Msgs with compose open change nothing outside compose | modal owns mouse |
| 'c' scoping | unit (update) | 8 | `c` on List/Projects opens no compose | detail-only entry |
| modal_area pure | unit | 5 | centered + clamped for small/large/narrow frames; never exceeds frame | layout seam |
| Modal render | render (TestBackend) | 5 | backdrop cell has DIM + dark bg; box ≈70% centered; title + hint + typed text in box | visual contract |
| Spawn reply mapping | unit (wiremock) | 2, 4 | 2xx -> CommentMutationOk; non-2xx -> CommentMutationErr (401 marker) | shell effect |

## Related

- ADR: [/adr/0024-modal-overlay-compose.md](/adr/0024-modal-overlay-compose.md)
- ADR: [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md)
- Issues: [/issues/0047-c3a-modal-primitive.md](/issues/0047-c3a-modal-primitive.md), [/issues/0048-c3b-compose-post-refresh.md](/issues/0048-c3b-compose-post-refresh.md)
- Fork base: `active-collab-cli` BDR 0024/0026
