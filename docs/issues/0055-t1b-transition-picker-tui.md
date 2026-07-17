---
type: Issue
title: "T1b — transition picker TUI: 's' opens a modal fetched from the workflow, Enter applies a field-free transition, the detail reloads from the server"
description: On the browse detail, 's' opens a transition-picker modal (reusing the C3a modal primitive) that dispatches list_transitions on open (loading state). j/k/arrows move a highlight; Enter on a field-free transition emits transition_issue then a server-truth RefreshDetail; Enter on a field-requiring transition shows a localized "requires fields" hint and does not write; Esc cancels. Empty transitions show a localized empty state. Fetch/execute failures close the picker with a localized error (401 = re-auth). The picker owns all input while open (no leakage, q no quit).
status: done
labels: [tui, transition, workflow, write, modal, mutation, parity]
blocked_by: 0054
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## T1b — transition picker TUI

Implements [ADR 0027](/adr/0027-status-transition-write-enablement.md) §3, §4, §5
and [BDR 0018](/bdr/0018-status-transition-behaviors.md) (all scenarios) on top of
the T1a client seam ([0054](/issues/0054-t1a-transition-client-seam.md)) and the
modal primitive ([ADR 0024](/adr/0024-modal-overlay-compose.md)).

Scope: `src/tui/model.rs`, `src/tui/view.rs`, `src/tui/shell.rs`,
`locales/pt_BR.json`, `tests/unit/tui.rs`, `tests/unit/tui/shell.rs`.

- **Model:** a typed picker overlay — `transition_picker: Option<TransitionPicker>`
  with a loading/loaded(Vec<Transition> + highlight index)/error state (mutually
  exclusive with `compose` and `confirm`). Msg: `OpenTransitions`,
  `TransitionsLoaded(Vec<Transition>)`, `TransitionsLoadErr(String)`,
  `TransitionMove(±1)`, `ApplyTransition`, `CancelTransitions`,
  `TransitionOk`/`TransitionErr(String)` (or reuse the mutation-result arms if
  they stay clean). Cmd: `LoadTransitions(key)`, `ExecTransition{key, id}` (and
  the existing `RefreshDetail`).
- **update():** `s` (Detail, loaded issue) → open picker + `LoadTransitions`;
  fetch reply populates or errors; move clamps; Enter on a field-free row →
  exactly one `ExecTransition`; Enter on a field-requiring row → localized hint,
  no Cmd; success → picker None + one `RefreshDetail` (no local status patch);
  failure → picker None + localized status (401 reauth); Esc → picker None.
  Extend the input-leakage guard so `transition_picker.is_some()` owns input.
- **view.rs:** render the picker modal (list of transitions with highlight;
  field-requiring rows annotated; loading/empty/error states) via `render_modal`,
  mutually exclusive with the compose/confirm overlays.
- **shell.rs:** `s` keymap; a picker-mode keymap (j/k/↑/↓ move, Enter apply, Esc
  cancel); `spawn_load_transitions` + `spawn_exec_transition` mirroring the
  comment spawns (401 → reauth marker); Cmd dispatch.
- **locales/pt_BR.json:** picker title, "requires fields", "no transitions
  available", and transition error/status strings.
- **Tests:** headless update() for BDR 0018 S1–S9 + a wiremock spawn test for the
  fetch + execute paths.

**Delivered 2026-07-16.** `transition_picker: Option<TransitionPicker>` with
state `Loading | Loaded{transitions, highlight, notice}`; `s` opens it +
`Cmd::LoadTransitions`; move clamps + clears notice; Enter on a field-free row →
one `Cmd::ExecTransition` (field-requiring → localized "requires fields" notice,
picker stays); `TransitionApplied` → close + one server-truth
`Cmd::RefreshDetail`; fetch/exec errors close + localized status (401 reauth);
Esc cancels. The picker is a **third overlay** alongside compose and confirm —
mutually exclusive, with `leaks_through_open_transitions` + `is_reply_msg`
extended so it owns input (q no quit) while its async replies pass through. TEA
purity kept: the `ModalContent` is built by `transition_picker_content` in
view.rs (model.rs stays ratatui-free); `modal.rs` and the T1a client seam
untouched. Reviewer: approved, 8/8 ACs, confidence 0.95. Deferred to the
C4-close chrome slice: a "Reply to @X"-style richer picker chrome / mouse-click
row activation.
