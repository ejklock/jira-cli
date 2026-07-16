---
type: ADR
title: "Comment actions in the browse detail — a comment-focus axis gates edit/delete (own only) + reply (any); edit reuses the compose modal, delete uses the modal's confirm buttons, reply posts a new comment; every mutation is server-truth"
description: Add a comment-focus axis to the browse detail ([ and ] move a highlight across ALL comments, mirroring the inline-link focus). The authenticated user's account_id (a one-shot myself fetch at browse entry) gates ownership. On a focused OWN comment, 'e' opens the compose in edit mode (buffer pre-filled from the comment body) and Ctrl+S PUTs via update_comment; 'd' opens a Sim/Não confirm modal (the modal primitive's second adapter) and confirm DELETEs. On ANY focused comment, 'r' opens the compose to post a NEW comment (Jira is flat — no native threading) seeded with an @mention of the focused author. Every mutation re-derives the thread from the server (Cmd::RefreshDetail) — never an optimistic local edit/delete/insert. Non-own edit/delete shows a localized "not your comment" hint.
status: Accepted
supersedes:
superseded_by:
tags: [tui, comments, write, edit, delete, reply, modal, focus, ownership, mutation]
timestamp: 2026-07-16T00:00:00Z
---

# 0026. Comment edit / delete / reply via a comment-focus axis

## Context

C1/C2/C3 landed the comment write surface: the seam
([ADR 0022](/adr/0022-comment-write-seam.md): `add_comment` / `update_comment`
/ `delete_comment` behind `JiraClient`, plus the plain-text→ADF builder), the
non-TTY `comment` command ([ADR 0023](/adr/0023-non-tty-comment-command.md)),
and the TUI compose modal + server-truth refresh
([ADR 0024](/adr/0024-modal-overlay-compose.md), consuming the reusable
`src/tui/modal.rs` primitive). ADR 0024 explicitly scoped **C4 as the modal's
second adapter** ("an edit variant arrives in C4"; "the C4 delete-confirm is
the second adapter — its deletion test").

C4 completes Group C parity with the fork base: **edit** and **delete** the
user's own comment, and **reply** to a comment. The write endpoints are already
enabled ([ADR 0015](/adr/0015-comment-write-enablement.md): POST/PUT/DELETE on
the comment endpoints only) — **no Constitution amendment is needed** (that gate
is reserved for a *different* write surface such as issue status transitions).

Two facts shape the design:

- **The detail has no per-comment addressability yet.** Comments render as a
  flat scrolled panel (`comments_panel` → `comment_card`); the only per-item
  focus that exists is the inline-link focus ([ADR 0011](/adr/0011-keyboard-inline-link-navigation-browse-detail.md):
  a `detail_focused_link` index + highlight). Acting on "a comment" first needs
  a way to point at one.
- **Jira Cloud comments are flat.** There is no native parent/child threading
  (unlike the chat-style "Reply" affordance the fork base mimicked). A "reply"
  is therefore a **new top-level comment**; the affordance's value is preserving
  the "responding to this person" context via an @mention.

## Decision

### 1. A comment-focus axis (foundation)

`Model` gains `detail_focused_comment: Option<usize>` — an index into
`issue.comments`, distinct from `detail_focused_link` (a different axis, a
different key). **`]` moves focus to the next comment, `[` to the previous**
(clamped at the ends; `None` when the thread is empty). The focused comment is
highlighted in the view exactly as the focused link is (theme highlight),
mirroring the ADR 0011 precedent. Focus cycles **all** comments, not only the
user's own (per the product decision below). Entering/leaving the detail resets
focus to `None`.

### 2. Ownership from a one-shot `myself` fetch

On browse entry the shell dispatches `Cmd::LoadMyself` once; the spawn calls the
existing `client.myself()` (`Myself { account_id, display_name }`) and replies
`Msg::MyselfLoaded(account_id)`, stored as `Model.current_account_id:
Option<String>`. A comment is **own** iff
`comment.author_account_id == Some(current_account_id)`. If the fetch fails
(offline / auth error) `current_account_id` stays `None` → **nothing is own** →
edit and delete are unavailable (safe degradation); reply still works (any
comment). The fetch is non-blocking — the browse paints before it lands.

### 3. Edit reuses the compose modal (C4a)

`Compose` gains a target discriminator:
`ComposeTarget = New | Edit { comment_id: String }` (default `New` keeps the
C3b path unchanged). **`e` on a focused OWN comment** opens the compose in edit
mode: the buffer is pre-filled with `adf_to_plain_text(comment.body)` and the
title is the localized "Edit comment". Ctrl+S on a non-empty buffer emits one
`Cmd::EditComment { key, comment_id, body }`; the shell spawn calls
`client.update_comment` and replies the existing `Msg::CommentMutationOk` /
`Msg::CommentMutationErr`. Success re-derives the thread (§6); failure keeps the
draft with a localized in-box error (401 → the E2 re-auth message), no refresh.
**`e` on a non-own focused comment** shows a localized "not your comment"
transient hint and opens no compose; `e` with no focused comment is a no-op.

**ADF fidelity trade.** The comment body is stored as serialized ADF (or plain
text); edit round-trips it through `adf_to_plain_text` → edit → `plain_text_to_adf`,
so rich formatting in the original is flattened to plain text — identical to how
compose already treats bodies (plain-text only). Accepted: the write surface is
deliberately plain-text (ADR 0022), and a lossless rich editor is out of scope.

### 4. Delete uses the modal's confirm buttons (C4b)

`Model` gains `confirm: Option<ConfirmDelete { comment_id: String }>`. **`d` on
a focused OWN comment** opens a confirm modal built from the primitive's
`ModalContent` **buttons** (Sim / Não — the modal's first button adapter, the
"deletion test" ADR 0024 promised). Confirm keymap: `y` / Enter / click-Sim →
`Cmd::DeleteComment { key, comment_id }`; `n` / Esc / click-Não → close, no Cmd.
Success re-derives the thread (§6; the comment is gone because the server no
longer returns it). Failure closes the confirm and surfaces a localized
transient error (401 → re-auth). **`d` on a non-own comment** shows the "not
your comment" hint. While the confirm modal is open the same input-leakage guard
as the compose applies (no key/mouse reaches the detail or list).

### 5. Reply posts a new comment with an @mention (C4c)

**`r` on ANY focused comment** opens the compose to post a **new** comment
(`ComposeTarget::New` — Jira is flat) seeded with an @mention of the focused
comment's author, built from `author_account_id` so the mention is a real Jira
mention (notifies the person), not a plain-text `@name`. The submit path passes
the mention account_id alongside the body so `plain_text_to_adf` (or a thin
wrapper) emits a leading ADF mention node. The exact seeding mechanics are
finalized in the C4c slice; this ADR fixes the decision (reply = mentioned new
comment, server-truth refresh) and the flat-comment rationale.

### 6. Server-truth refresh for every mutation

Edit, delete, and reply all re-use the C3b `Cmd::RefreshDetail(key)` path (a
cache-busting `spawn_load_detail`, single-flight) on success. **No optimistic
mutation anywhere**: the model never edits, removes, or inserts a comment
locally — the thread only changes when the fresh server payload replaces it
(the server owns id / author / timestamp / normalized ADF). This is the ADR 0024
§5 invariant, extended to all three verbs.

### 7. Chrome

When a comment is focused, the detail footer hint gains the action keys
(`[ ] focus · e edit · d delete · r reply`), with the own-only actions
(`e` / `d`) shown only when the focused comment is the user's own. The compose
title switches between "New comment" / "Edit comment"; the confirm modal owns
its Sim/Não hint. All strings are English-source keys with pt-BR catalog
entries ([ADR 0006](/adr/0006-i18n-interpolation-contract.md)).

## Alternatives considered

- **Focus only the user's own comments.** Rejected (product decision
  2026-07-16): focus navigates *all* comments; edit/delete are *gated* to own
  with a "not yours" hint, so the thread stays fully navigable and reply reaches
  any comment.
- **Native reply threading.** Impossible — Jira Cloud comments are flat; a reply
  is a new top-level comment. The @mention preserves the responding-to context.
- **Optimistic edit/delete (mutate the local thread, then reconcile).**
  Rejected — the server normalizes ADF and owns timestamps; one refresh
  round-trip buys guaranteed consistency (the ADR 0024 §5 trade, extended).
- **Skip client-side ownership (let the server 403).** Rejected — pre-gating
  with the already-present `myself` seam gives a clear, immediate UX; the server
  check remains the backstop.
- **A dedicated "comment focus mode" (enter, then j/k).** Rejected (YAGNI) —
  a persistent `[`/`]` axis is simpler and never captures the scroll keys.

## Consequences

**Positive:** the modal primitive earns its second adapter (confirm buttons);
the compose gains an edit target with almost no new machinery (same submit /
refresh / error arms, a different Cmd + seam); a reusable comment-focus axis
lands (a future "react/like" affordance could hang off it). **Trade-offs:** an
extra `myself` round-trip at browse entry; ADF rich formatting is flattened on
edit (plain-text write surface); `view()` gains a comment-highlight pass and a
confirm-modal branch.

## Related

- ADR: [/adr/0024-modal-overlay-compose.md](/adr/0024-modal-overlay-compose.md)
  (the modal primitive + compose + server-truth refresh),
  [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md)
  (update/delete seam + author identity),
  [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md)
  (§4 server-truth, write surface),
  [/adr/0011-keyboard-inline-link-navigation-browse-detail.md](/adr/0011-keyboard-inline-link-navigation-browse-detail.md)
  (the focus-index + highlight precedent)
- BDR: [/bdr/0017-comment-action-behaviors.md](/bdr/0017-comment-action-behaviors.md)
- Issues: [/issues/0051-c4a-comment-focus-edit.md](/issues/0051-c4a-comment-focus-edit.md),
  [/issues/0052-c4b-delete-confirm-modal.md](/issues/0052-c4b-delete-confirm-modal.md),
  [/issues/0053-c4c-reply-mention.md](/issues/0053-c4c-reply-mention.md)
- Fork base: `active-collab-cli` comment edit/delete/reply parity
