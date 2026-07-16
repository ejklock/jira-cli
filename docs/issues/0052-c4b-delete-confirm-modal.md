---
type: Issue
title: "C4b — delete own comment via a Sim/Não confirm modal (the modal primitive's button adapter → delete_comment, server-truth refresh)"
description: Model gains confirm: Option<ConfirmDelete{comment_id}>. 'd' on a focused OWN comment opens a confirm modal built from the ModalContent buttons (Sim/Não) with a localized prompt; y/Enter/click-Sim emits Cmd::DeleteComment{key,comment_id} → shell spawn → client.delete_comment → CommentMutationOk/Err; n/Esc/click-Não closes with no write. Success closes + one cache-busting RefreshDetail (the comment is absent from the fresh thread); failure closes the confirm and surfaces a localized transient error (401 reauth). 'd' on a non-own comment shows the "not your comment" hint. Full key/mouse leakage guards while the confirm modal is open.
status: done
labels: [tui, comments, write, delete, modal, confirm, parity]
blocked_by: 0045, 0047, 0051
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## C4b — delete confirm modal

Implements [ADR 0026](/adr/0026-comment-edit-delete-reply-focus.md) §4, §6 and
[BDR 0017](/bdr/0017-comment-action-behaviors.md) S6–S7, S9–S10 (delete slice of
the matrix). The confirm modal is the **second adapter** of the C3a modal
primitive and the first to use its optional buttons (the deletion test ADR 0024
promised).

Scope: `src/tui/model.rs`, `src/tui/view.rs`, `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui_render.rs`,
`tests/unit/tui/shell.rs`.

**Delivered 2026-07-16.** `Model.confirm: Option<ConfirmDelete{comment_id}>`;
`d` on a focused OWN comment opens a Sim/Não confirm modal (the C3a primitive's
first button consumer), `y`/Enter confirms → `Cmd::DeleteComment` →
`delete_comment` → server-truth `RefreshDetail` (no local removal), `n`/Esc
cancels. `CommentMutationOk`/`Err` are now context-aware across compose/confirm.
Mouse-click on the buttons and the footer action-key chrome are deferred to the
C4-close chrome slice; the confirm is keyboard-driven for now.
