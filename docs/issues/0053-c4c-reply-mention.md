---
type: Issue
title: "C4c — reply to a focused comment: a new comment seeded with an @mention of the author (Jira is flat → reply = new top-level comment)"
description: 'r' on ANY focused comment opens the compose as a NEW comment (ComposeTarget::New) seeded with an ADF @mention of the focused comment's author (built from author_account_id, so it is a real Jira mention that notifies the person — not a plain-text @name). The submit path carries the mention account_id alongside the body so plain_text_to_adf (or a thin wrapper) emits a leading mention node; Ctrl+S emits the existing add (SubmitComment) Cmd → client.add_comment → server-truth refresh. Finalizes the reply seeding mechanics left open by ADR 0026 §5.
status: todo
labels: [tui, comments, write, reply, mention, parity]
blocked_by: 0045, 0047, 0051
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## C4c — reply as a mentioned new comment

Implements [ADR 0026](/adr/0026-comment-edit-delete-reply-focus.md) §5 and
[BDR 0017](/bdr/0017-comment-action-behaviors.md) S8. Jira Cloud comments are
flat: a reply is a new top-level comment; the @mention preserves the
responding-to-this-person context and notifies the author natively.

Open design point to finalize in this slice: the exact ADF mention-seeding shape
(a leading `mention` node from `author_account_id`) and whether the mention is
editable/removable in the buffer before submit.

Scope: `src/tui/model.rs`, `src/client.rs` (ADF mention builder), `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui/shell.rs`.
