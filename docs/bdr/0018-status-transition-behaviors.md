---
type: BDR
title: "Status transition on the browse detail — s opens a transition picker fetched from the workflow, Enter moves the issue, the detail reloads from the server"
description: On the browse detail of a loaded issue, 's' opens a transition-picker modal that fetches the available workflow transitions for the current state. j/k/arrows move a highlight; Enter executes the highlighted field-free transition (POST), then the detail reloads from the server so the new status shows; Esc cancels with no write. A transition that requires screen fields is listed but not executable (a localized "requires fields" hint). An issue with no available transitions shows a localized empty state. Fetch/execute failures close the picker with a localized error (401 = re-auth). While the picker is open no key/mouse input reaches the detail or list.
status: Accepted
superseded_by:
supersedes:
tags: [tui, transition, workflow, write, modal, mutation]
timestamp: 2026-07-16T00:00:00Z
---

# 0018. Status transition behaviors

## Context

The first slice of Constitution [Amendment 2](/constitution.md), implementing
[ADR 0027](/adr/0027-status-transition-write-enablement.md) on top of the modal
primitive ([ADR 0024](/adr/0024-modal-overlay-compose.md)) and the server-truth
`RefreshDetail` path ([ADR 0015](/adr/0015-comment-write-enablement.md) §4).
Transitions are workflow- and state-specific, so the legal moves are **read from
the API** each time the picker opens; only field-free transitions execute in v1.

## Textual Description

On the **browse detail** of a loaded issue:

- **`s`** ("status") opens the **transition picker** — a modal that, on open,
  fetches the available transitions for the issue's current workflow state
  (a **loading** state while the fetch is in flight).
- Once loaded, the modal lists each transition by **name** (and its target
  status). `j`/`k`/arrows move a highlight; the footer shows `↑↓ move · ⏎ apply ·
  esc cancel`.
- **Enter on a field-free transition** executes exactly one transition (POST),
  closes the picker, and the detail **reloads from the server** so the new status
  is shown — never an optimistic local status change.
- **Enter on a transition that requires screen fields** does **not** write: it
  surfaces a localized "requires fields — not supported in the CLI yet" hint and
  leaves the picker open. Such rows are visibly annotated as non-executable.
- **Esc** cancels the picker with no write.
- An issue with **no available transitions** shows a localized "no transitions
  available" empty state (Esc closes it).
- A **fetch failure** closes the picker with a localized error; an **execute
  failure** (non-2xx) closes the picker with a localized error on the thin status
  line and performs no refresh. A **401** on either follows the re-auth contract.
- While the picker is open it **owns all input**: focus/nav keys and mouse events
  are inert and `q` does not quit — like the compose and delete-confirm modals.

## Scenarios

- **S1 — open + fetch.** `s` on Detail with a loaded issue opens the picker and
  dispatches exactly one transitions-fetch Cmd; the picker shows the loading
  state until the reply lands.
- **S2 — loaded list.** The fetch reply populates the picker with the transitions
  (name + target status); field-free rows are executable, field-requiring rows
  are annotated non-executable.
- **S3 — execute field-free.** Enter on a field-free highlighted transition emits
  exactly one transition-execute Cmd for that transition id; on success the
  picker closes and exactly one `RefreshDetail(key)` is emitted (no local status
  patch).
- **S4 — block field-requiring.** Enter on a field-requiring highlighted
  transition sets the localized "requires fields" hint, emits no Cmd, and leaves
  the picker open.
- **S5 — cancel.** Esc closes the picker with no Cmd and no status change.
- **S6 — execute failure.** A transition-execute error reply closes the picker,
  sets a localized transient error status (401 → the re-auth message), and emits
  zero refresh Cmds.
- **S7 — fetch failure.** A transitions-fetch error reply closes the picker and
  sets a localized error status.
- **S8 — empty.** A fetch that returns no transitions shows the localized "no
  transitions available" empty state; Enter is inert, Esc closes.
- **S9 — input leakage.** While the picker is open, `[`/`]`/`j`/`k`/`q`/`p`/`e`/
  `d`/`r`/`c` and mouse events leave the detail/list state unchanged and `q` does
  not quit; only move/apply/cancel react.

## Test Matrix

| Scenario | Trigger | Expected | Verify |
|---|---|---|---|
| S1 | `s` on Detail (loaded issue) | picker open + one fetch Cmd, loading state | headless update() |
| S2 | fetch reply (mixed transitions) | list populated; field-free executable, field-requiring annotated | headless update() + render |
| S3 | Enter on field-free row | one execute Cmd(id) → close + one RefreshDetail(key) | headless update() |
| S4 | Enter on field-requiring row | localized "requires fields" hint, no Cmd, picker stays | headless update() |
| S5 | Esc | picker None, no Cmd | headless update() |
| S6 | execute error reply (incl. 401) | picker None + localized error (401 reauth), zero refresh | headless update() |
| S7 | fetch error reply | picker None + localized error | headless update() |
| S8 | fetch returns [] | localized empty state; Enter inert; Esc closes | headless update() + render |
| S9 | keys/mouse while picker open | detail/list unchanged, q no quit | headless update() |
| wire | execute a transition | POST /rest/api/3/issue/{key}/transitions body {transition:{id}} | wiremock |
| wire | fetch transitions | GET .../transitions?expand=transitions.fields parsed to the domain type | wiremock |

## Related

- ADR: [/adr/0027-status-transition-write-enablement.md](/adr/0027-status-transition-write-enablement.md)
- Modal primitive: [/adr/0024-modal-overlay-compose.md](/adr/0024-modal-overlay-compose.md)
- Server-truth refresh: [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md) §4
- Constitution: [/constitution.md](/constitution.md) (Amendment 2)
