---
type: Issue
title: "C4c — reply to a focused comment: a new comment seeded with an @mention of the author (Jira is flat → reply = new top-level comment)"
description: 'r' on ANY focused comment opens the compose as a NEW comment (ComposeTarget::New) seeded with an ADF @mention of the focused comment's author (built from author_account_id, so it is a real Jira mention that notifies the person — not a plain-text @name). The submit path carries the mention account_id alongside the body so plain_text_to_adf (or a thin wrapper) emits a leading mention node; Ctrl+S emits the existing add (SubmitComment) Cmd → client.add_comment → server-truth refresh. Finalizes the reply seeding mechanics left open by ADR 0026 §5.
status: done
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

Scope: `src/tui/model.rs`, `src/client.rs` (ADF mention builder), `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui/shell.rs`.

**Delivered 2026-07-16.** The open design point is resolved: rather than reuse
`ComposeTarget::New` with a buffer-seeded `@name`, the reply carries the mention
**structurally** on a distinct `ComposeTarget::Reply { mention_account_id,
mention_display }` and the compose buffer starts **empty** — so the mention
survives buffer edits and posts as a real Jira `mention` ADF node (it notifies
the author; a plain-text `@name` would not). `submit_compose_cmd` maps `Reply →
Cmd::ReplyComment`; `spawn_reply_comment` calls a new `reply_comment(key,
account_id, display, body)` client seam. In `client.rs` the paragraph-content
builder was extracted (`plain_text_content`) so `plain_text_to_adf` stays
byte-identical while `mention_adf` prepends `[mention, text " ", …body]`. Reply
is **not** ownership-gated (`'r'` works on any focused comment, including your
own); it reuses the context-aware `CommentMutationOk`/`Err` arms + server-truth
`RefreshDetail` (no new result `Msg`). The wire ADF (mention node first, then the
body) is asserted by a wiremock `body_json` matcher. Reviewer: approved, 8/8 ACs,
confidence 0.97. A `Reply to @X` modal-title/footer chrome hint is deferred to
the C4-close chrome slice.
