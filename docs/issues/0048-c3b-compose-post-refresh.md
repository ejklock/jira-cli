---
type: Issue
title: "C3b — comment compose through the modal: 'c' opens, Ctrl+S posts, server-truth refresh"
description: Model gains compose state + keymap (c opens on detail; chars/Enter-newline/Backspace; Ctrl+S submits non-empty; Esc discards) with full key/mouse leakage guards; Cmd::SubmitComment -> shell spawn -> client.add_comment -> CommentMutationOk/Err; Ok clears compose + exactly one cache-busting detail refresh (no optimistic mutation), Err preserves the draft with a localized in-box error (401 reauth); view renders the compose through the C3a modal.
status: todo
labels: [tui, comments, write, compose, parity]
blocked_by: 0045, 0047
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## C3b — compose + POST + server-truth refresh

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-C1 (create) per
[ADR 0024](/adr/0024-modal-overlay-compose.md), behaviors
[BDR 0015](/bdr/0015-comment-compose-behaviors.md) S1–S8.

Scope: `src/tui/model.rs`, `src/tui/view.rs`, `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui_render.rs`.
