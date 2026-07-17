---
type: BDR
title: "Comment actions on the browse detail — [ ] focus a comment, e edits your own (pre-filled compose), d deletes your own (Sim/Não confirm), r replies to anyone (mentioned new comment); every mutation reloads the thread from the server"
description: On the browse detail, [ and ] move a highlight across all comments. With a comment focused, 'e' opens the compose pre-filled from your OWN comment and Ctrl+S saves the edit; 'd' opens a Sim/Não confirm modal and confirming deletes your OWN comment; 'r' opens the compose to post a NEW comment mentioning the focused author (Jira is flat). Editing/deleting a comment that is not yours shows a "not your comment" hint and does nothing. Every successful mutation reloads the thread from the server (no optimistic local edit/delete/insert); failures keep any draft and surface a localized error (401 = re-auth). While the edit compose or the confirm modal is open no key/mouse input reaches the detail or list.
status: Accepted
superseded_by:
supersedes:
tags: [tui, comments, write, edit, delete, reply, focus, ownership, modal, mutation]
timestamp: 2026-07-16T00:00:00Z
---

# 0017. Comment action behaviors (edit / delete / reply)

## Context

C4 of Group C, implementing [ADR 0026](/adr/0026-comment-edit-delete-reply-focus.md)
on top of the compose modal ([ADR 0024](/adr/0024-modal-overlay-compose.md) /
[BDR 0015](/bdr/0015-comment-compose-behaviors.md)) and the write seam
([ADR 0022](/adr/0022-comment-write-seam.md)). Sliced C4a (focus + ownership +
edit) → C4b (delete confirm) → C4c (reply). Jira comments are flat: a reply is a
new top-level comment carrying an @mention of the focused author.

## Textual Description

On the **browse detail** of a loaded issue:

- **`]`** focuses the next comment, **`[`** the previous (clamped; nothing to
  focus when the thread is empty). The focused comment is highlighted like a
  focused link. Focus resets when leaving the detail.
- With a comment focused, the footer shows `[ ] focus  e edit  d delete  r
  reply  s status` (finalized in [issue 0056](/issues/0056-c4d-detail-footer-action-hints.md),
  keeping the established double-space footer separator).
  The footer **advertises** the actions whenever a comment is focused; ownership
  is **enforced at invocation** — `e`/`d` on a comment that is not **yours** (its
  author account id equals the authenticated user's, learned from a one-shot
  `myself` fetch at browse entry) surface the localized "not your comment" hint
  and do nothing. This advertise-then-enforce choice keeps the footer a single
  string per `FooterMode` (ADR 0014 §5) with no per-ownership variant.
- **`e` on your own comment** opens the compose in **edit mode**: the box title
  is "Edit comment" and the buffer is **pre-filled** with the comment's current
  text. Editing and Ctrl+S behave like compose, but a non-empty save emits an
  **edit** Cmd (PUT via `update_comment`) for that comment id. `e` on a comment
  that is **not yours** shows a localized "not your comment" hint and opens no
  compose.
- **`d` on your own comment** opens a **confirm modal** with **Sim / Não**
  buttons and a localized prompt. `y` / Enter / Sim deletes (DELETE via
  `delete_comment`); `n` / Esc / Não closes with no write. `d` on a comment that
  is **not yours** shows the "not your comment" hint.
- **`r` on any focused comment** opens the compose to post a **new** comment,
  seeded with an @mention of the focused author (Jira has no nested threads).
- On **success** of any mutation the modal/compose closes and the thread
  **reloads from the server** (cache-busting) — the edit, deletion, or new reply
  appears with real server data. **No locally-fabricated or locally-mutated
  comment ever renders.**
- On **failure** an edit keeps its draft with a localized in-box error; a delete
  closes the confirm and surfaces a transient error; a 401 shows the re-auth
  message. No refresh happens on failure.
- While the edit compose or the confirm modal is open, **no input leaks**:
  list/detail keys and mouse events do not reach the machinery behind the
  backdrop; `q` does not quit.

## Scenarios

**Scenario 1: focus moves across all comments** — Given a detail with three
comments and no focus, When the user presses `]` twice then `[` once, Then the
focused-comment index is 1 (second comment), clamped within `0..len`, and the
focused comment is highlighted. `]` past the last / `[` before the first clamps.

**Scenario 2: ownership is learned from myself** — Given the authenticated user
has account id `A`, When the `myself` fetch replies, Then `current_account_id`
is `Some(A)` and a comment authored by `A` is "own" while a comment authored by
`B` is not; if the fetch never lands, no comment is own.

**Scenario 3: edit opens a pre-filled compose for your own comment** — Given a
focused OWN comment with body "hello", When the user presses `e`, Then the
compose opens in edit mode targeting that comment id, the buffer equals "hello",
and the title key is "Edit comment".

**Scenario 4: edit saves via the edit Cmd** — Given an open edit compose with a
non-empty buffer, When the user presses Ctrl+S, Then exactly one
`Cmd::EditComment { key, comment_id, body }` is emitted with the open key and the
focused comment id, and the status shows Submitting; an empty buffer emits no
Cmd.

**Scenario 5: edit success reloads, failure keeps the draft** — Given a submitted
edit, When it succeeds, Then the compose closes and exactly one cache-busting
`Cmd::RefreshDetail` for the open key is emitted with no locally-mutated comment;
When it fails, Then the buffer is preserved, a localized error is set (401 → the
re-auth message), and zero refresh Cmds are emitted.

**Scenario 6: edit/delete are gated to your own** — Given a focused comment that
is NOT yours, When the user presses `e` or `d`, Then no compose and no confirm
modal open, a localized "not your comment" hint is shown, and no Cmd is emitted.

**Scenario 7: delete asks for confirmation** — Given a focused OWN comment, When
the user presses `d`, Then a confirm modal with Sim/Não buttons opens; When the
user presses Não / Esc, Then it closes with no Cmd; When the user presses Sim /
Enter / `y`, Then exactly one `Cmd::DeleteComment { key, comment_id }` is emitted
and on success exactly one `Cmd::RefreshDetail` for the open key (the deleted
comment is absent from the fresh thread).

**Scenario 8: reply posts a mentioned new comment** — Given a focused comment by
author `B` (account id `Bacct`), When the user presses `r`, Then the compose
opens as a NEW comment seeded so the submitted body carries an ADF mention of
`Bacct`, and Ctrl+S emits a `SubmitComment` (add) Cmd — not an edit — followed by
the server-truth refresh on success.

**Scenario 9: no input leakage while a comment modal is open** — Given an open
edit compose or an open delete-confirm, When the user presses detail/list keys
(j, k, q, p, Tab, `[`, `]`) or clicks/scrolls the backdrop, Then the detail
scroll, comment focus, selection, links, and list state are unchanged and `q`
does not quit.

**Scenario 10: confirm modal renders centered with buttons** — Given an open
delete-confirm, When the frame renders, Then backdrop cells carry DIM + the dark
backdrop background, the box is centered (clamped on small terminals), and the
localized prompt plus the Sim and Não button labels render inside the box.

## Test Design

Pure `update()` drives S1–S9 headlessly (focus arithmetic, ownership predicate,
edit/delete/reply Cmd emission, gating, leakage guards); `TestBackend` buffers
prove S3/S10 (edit title + pre-filled buffer in the box; confirm prompt + Sim/Não
labels + DIM backdrop). The shell spawns are covered by wiremock write tests
(reply mapping Ok/Err for PUT and DELETE), mirroring the existing spawn tests.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Focus nav clamps | unit (update) | 1 | `]`/`[` move index within `0..len`, clamp at ends, empty → None | focus axis |
| Ownership predicate | unit (update) | 2 | comment by A is own iff current_account_id==Some(A); None → nothing own | ownership |
| Edit pre-fill | unit (update) | 3 | `e` on own → compose Edit{id}, buffer == body text, title key "Edit comment" | edit open |
| Edit emits edit Cmd | unit (update) | 4 | Ctrl+S non-empty → exactly one EditComment{key,id,body}+Submitting; empty → none | edit submit gate |
| Edit Ok/Err | unit (update) | 5 | Ok → compose None + exactly one RefreshDetail, no local mutation; Err → buffer intact, Error(401→reauth), zero refresh | server-truth edit |
| Own-gating | unit (update) | 6 | `e`/`d` on non-own → no modal, hint set, no Cmd | ownership gate |
| Delete confirm | unit (update) | 7 | `d` own → confirm Some; Não/Esc → None no Cmd; Sim/Enter/y → exactly one DeleteComment; Ok → one RefreshDetail | delete flow |
| Reply mention | unit (update) | 8 | `r` → compose New; submit body carries mention(Bacct); emits add (SubmitComment) not edit | reply-as-new |
| No leakage | unit (update) | 9 | with edit/confirm open: j/k/q/p/Tab/`[`/`]`/mouse leave scroll/focus/selection/list unchanged; q no quit | modal owns input |
| Confirm render | render (TestBackend) | 10 | backdrop DIM+dark bg; centered box; prompt + Sim + Não labels in box | confirm visual |
| Edit render | render (TestBackend) | 3 | edit modal shows "Edit comment" + pre-filled buffer text | edit visual |
| PUT/DELETE spawn | unit (wiremock) | 5, 7 | 2xx → CommentMutationOk; non-2xx → CommentMutationErr (401 marker) for update_comment and delete_comment | shell effect |

## Related

- ADR: [/adr/0026-comment-edit-delete-reply-focus.md](/adr/0026-comment-edit-delete-reply-focus.md)
- ADR: [/adr/0024-modal-overlay-compose.md](/adr/0024-modal-overlay-compose.md),
  [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md)
- Issues: [/issues/0051-c4a-comment-focus-edit.md](/issues/0051-c4a-comment-focus-edit.md),
  [/issues/0052-c4b-delete-confirm-modal.md](/issues/0052-c4b-delete-confirm-modal.md),
  [/issues/0053-c4c-reply-mention.md](/issues/0053-c4c-reply-mention.md)
