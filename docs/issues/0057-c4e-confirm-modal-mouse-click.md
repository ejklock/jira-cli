---
type: Issue
title: "C4e — mouse-click activation of the delete-confirm Sim/Não buttons: wire the modal's ButtonTarget click geometry (ADR 0024 §2d) into shell mouse resolution"
description: The delete-confirm modal already renders Sim/Não buttons and modal::render_modal already returns their ButtonTarget click geometry (ADR 0024 §2d), but no consumer wires it — a mouse click on a button does nothing (the backdrop is fully inert while the confirm is open). This slice makes a left-click on the Sim button emit Msg::ConfirmDeleteYes and on the Não button Msg::ConfirmDeleteNo, keyboard behavior (y/Enter/n/Esc) unchanged, matching the codebase's pure recompute hit-test pattern (list_click_card / detail_link_at). The button geometry is extracted into a pure absolute-coordinate function in modal.rs shared by render_modal (single source, no drift); model.rs stays ratatui-free.
status: done
labels: [tui, mouse, modal, confirm, comments, chrome, parity]
blocked_by: 0052
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## C4e — mouse-click activation of the delete-confirm buttons

Finalizes the delete-confirm chrome deferred from [C4b](/issues/0052-c4b-delete-confirm-modal.md):
the Sim/Não buttons become clickable. Implements the click contract
[ADR 0024 §2d](/adr/0024-modal-overlay-compose.md) already designed
(`ModalButton.id` is the opaque token a click matches; `render_modal` returns
`ButtonTarget` geometry) and adds [BDR 0017](/bdr/0017-comment-action-behaviors.md)
S11. No new ADR — it wires an already-decided mechanism.

Scope: `src/tui/modal.rs`, `src/tui/view.rs`, `src/tui/shell.rs`,
`tests/unit/tui/modal.rs`, `tests/unit/tui_render.rs`, `tests/unit/tui/shell.rs`.

- **modal.rs:** extract a pure `pub fn button_targets(frame_area, content) ->
  Vec<ButtonTarget>` returning ABSOLUTE frame coordinates (via the existing
  `desired_size` → `modal_area` → `block.inner` → `split_rows` → per-button
  x-advance layout). `render_modal`/`render_buttons` reuse the same layout so
  the rendered buttons and the hit-test geometry can never drift. No change to
  `render_modal`'s public `ModalRender` output (still modal-relative).
- **view.rs:** a pure `confirm_button_at(frame_area, x, y) -> Option<String>`
  that builds `confirm_modal_content()`, calls `modal::button_targets`, and
  returns the id of the button whose absolute rect contains the click —
  mirroring `detail_link_at`.
- **shell.rs:** split the `confirm_active` mouse branch (today `confirm_active ||
  compose_active || transitions_active => None`): a left-button Down over the
  confirm resolves through `confirm_button_at` and maps `"yes" ->
  Msg::ConfirmDeleteYes`, `"no" -> Msg::ConfirmDeleteNo`; a click on the
  backdrop/body (no button hit) stays inert, and scroll/drag/release over the
  confirm stay inert. compose and transitions remain fully mouse-inert.
- **Keyboard unchanged:** `y`/`Enter`/`n`/`Esc` still resolve via
  `map_key_in_confirm_mode`; `q` does not quit while the confirm is open.
- **Tests:** `button_targets` absolute geometry (two buttons, row/x-advance);
  `confirm_button_at` returns `"yes"`/`"no"` on the respective labels and `None`
  on the backdrop; the shell dispatches ConfirmDeleteYes/No on a button click and
  stays inert on a backdrop click; compose/transitions mouse still inert.

**Delivered 2026-07-16.** `modal.rs` extracts the per-button x-advance loop into a
private `layout_button_targets` reused by `render_buttons` **and** a new
`pub fn button_targets(frame_area, content) -> Vec<ButtonTarget>` returning
absolute frame coords (single geometry source — the rendered cells and the
hit-test cannot drift; `render_modal`'s `ModalRender` output stays
modal-relative and unchanged). `view::confirm_button_at(frame_area, x, y) ->
Option<String>` is a pure hit-test mirroring `detail_link_at`. `shell.rs` splits
the confirm mouse branch: a left Down over the confirm routes through
`resolve_confirm_mouse` → `confirm_msg_for_id` to `Msg::ConfirmDeleteYes`/`No`;
backdrop clicks and scroll/drag/release stay inert; compose and transition-picker
remain fully mouse-inert; the confirm keyboard path and `q`-no-quit are unchanged.
`model.rs` untouched (stays ratatui-free, ADR 0007 §6). A boundary test one column
left of the Sim button kills the off-by-one hit-test mutant. Reviewer: approved,
7/7 ACs, confidence 0.95.
