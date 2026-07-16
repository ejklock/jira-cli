---
type: Issue
title: "C4a — comment focus axis + myself ownership + edit own comment (pre-filled compose → update_comment, server-truth refresh)"
description: Model gains detail_focused_comment (index) moved by [ and ] with a highlight across ALL comments (mirroring the inline-link focus), plus current_account_id from a one-shot myself fetch at browse entry (Cmd::LoadMyself → Msg::MyselfLoaded). Compose gains ComposeTarget (New | Edit{comment_id}); 'e' on a focused OWN comment opens the compose pre-filled with the comment body and titled "Edit comment"; Ctrl+S emits Cmd::EditComment{key,comment_id,body} → shell spawn → client.update_comment → CommentMutationOk/Err; Ok clears compose + one cache-busting RefreshDetail (no local mutation), Err keeps the draft with a localized in-box error (401 reauth). 'e' on a non-own comment shows a "not your comment" hint and opens nothing. Full key/mouse leakage guards while the edit compose is open.
status: done
labels: [tui, comments, write, edit, focus, ownership, parity]
blocked_by: 0045, 0047, 0048
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## C4a — comment focus + ownership + edit

Implements [ADR 0026](/adr/0026-comment-edit-delete-reply-focus.md) §1–§3, §6–§7
and [BDR 0017](/bdr/0017-comment-action-behaviors.md) S1–S6, S9 (edit slice of
the matrix). Reuses the compose submit/refresh/error machinery from C3b — edit
differs only in the target discriminator, the emitted Cmd, and the seam called
(`update_comment`).

Scope: `src/tui/model.rs`, `src/tui/view.rs`, `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui_render.rs`,
`tests/unit/tui/shell.rs`.

Foundation shared with C4b/C4c: the comment-focus axis and the `myself`
ownership identity land here; delete (C4b) and reply (C4c) build on them.

**Delivered 2026-07-16** in two coder passes under this issue: (1) the
`[`/`]` comment-focus axis + `detail_focused_comment` highlight + `myself`
one-shot fetch (`current_account_id`) + the pure `is_own_comment` predicate;
(2) `Compose.target` (`New` | `Edit{comment_id}`), `e` opens the compose
pre-filled via `adf_to_plain_text` and titled "Edit comment", Ctrl+S emits
`Cmd::EditComment` → `update_comment` → server-truth `RefreshDetail`, non-own
`e` shows a localized "Not your comment" hint. Footer action-key chrome is
deferred to the C4 close.
