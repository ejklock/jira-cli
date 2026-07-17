---
type: ADR
title: "Status transition write enablement — GET/POST on the Jira transitions endpoints only, field-free transitions in v1"
description: Enable moving an issue through its workflow from the browse Detail via a transition-picker modal, as the single new write surface allowed by Constitution Amendment 2. Available transitions are fetched on open; Enter executes a field-free transition; transitions that require screen fields are shown but not performed. Server-truth refresh after every transition; token host isolation extends to the transition requests; everything else stays read-only.
status: Accepted
supersedes:
superseded_by:
tags: [write, transition, workflow, api, security, parity, tui]
timestamp: 2026-07-16T00:00:00Z
---

# 0027. Status transition write enablement

## Context

The constitution declared v1 read-only, with write "a deliberate later slice,
behind its own ADR". [Amendment 1](/constitution.md) narrowed the write
exclusion to comment writes ([ADR 0015](/adr/0015-comment-write-enablement.md));
comment create/edit/delete/reply landed (Group C). The parity program
([PRD 0003](/prd/0003-active-collab-parity.md)) next brings the fork base's
**status transition** feature: moving an issue through its workflow (To Do → In
Progress → Done). Constitution **Amendment 2** (2026-07-16) narrows the write
exclusion again to add exactly the transition endpoints. This ADR fixes HOW.

Jira transitions are workflow- and state-specific: the set of legal moves for an
issue depends on its current status and the project's workflow, so it must be
**read from the API**, not hard-coded. Some transitions open a screen requiring
fields (e.g. a resolution on Done); executing those needs a dynamic form far
larger than this slice.

## Decision

1. **Write surface = the Jira Cloud transition endpoints, nothing else.**
   `GET /rest/api/3/issue/{key}/transitions?expand=transitions.fields` (read the
   available transitions for the current state, plus whether each opens required
   fields) and `POST /rest/api/3/issue/{key}/transitions` with
   `{ "transition": { "id": <id> } }` (execute one). The client trait
   ([ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md)) grows
   `list_transitions(key) -> Vec<Transition>` and
   `transition_issue(key, transition_id) -> ()`; no other non-GET method is
   added. Enforced by the client request-surface unit test (the constitution's
   falsifiable clause).
2. **Field-free transitions only in v1.** The `expand=transitions.fields` payload
   reveals whether a transition requires fields. A transition with **any required
   field** is listed but rendered non-executable (a localized "requires fields —
   not supported in the CLI yet" annotation); only field-free transitions POST.
   This keeps the write surface minimal and honest — no silent partial writes.
3. **UI = a transition-picker modal, reusing the C3a modal primitive.** `s`
   ("status") on the browse Detail opens the picker, which fetches the available
   transitions on open (a loading state). `j`/`k`/arrows move a highlight; Enter
   executes the highlighted **field-free** transition; Esc cancels. There is **no
   separate confirm step** — the picker selection is the deliberate act and a
   workflow move is reversible (re-open the picker to move back). Enter on a
   field-requiring row surfaces the "requires fields" hint and performs no write.
4. **Server-truth refresh.** After a successful transition, re-fetch the issue
   (busting cache) via the C3b `Cmd::RefreshDetail(key)` path and re-render from
   the server payload — never patch the local status optimistically (the ADR 0015
   §4 / ADR 0024 §5 invariant, extended to the transition verb).
5. **Failure semantics** mirror [ADR 0015](/adr/0015-comment-write-enablement.md)
   §5: non-2xx → the transition is not retried, the picker closes with the error
   on the thin status line; a fetch failure closes the picker with a localized
   error; 401 follows the R-E2 re-auth contract. An issue with **no** available
   transitions shows a localized "no transitions available" state.
6. **Token host isolation extends to the transition requests** — both the GET and
   the POST reuse the host-pinned client; the isolation test suite gains
   transition-path cases.

## Alternatives considered

- **A non-TTY `transition` command now.** Deferred — the first slice is a TUI
  vertical (picker → execute → refresh). Amendment 2 permits a non-TTY
  `transition` command; it is a follow-up issue, mirroring how the non-TTY
  `comment` command followed the TUI compose.
- **A separate confirm modal before executing.** Rejected — double friction for a
  reversible workflow move; the picker selection is already the deliberate step
  (contrast delete, which is destructive and keeps the Sim/Não confirm).
- **Supporting field-requiring transitions (resolution screens) now.** Deferred —
  needs a dynamic field form (a much larger surface); v1 fences to field-free
  transitions and is explicit about the rest.
- **Hard-coding a status list / optimistic local status change.** Rejected —
  transitions are workflow-specific (must be read) and the server is the
  authority (server-truth refresh, immune to divergence).

## Consequences

**Positive:** the "move the ticket without leaving the terminal" flow lands; the
write surface stays minimal, testable, and constitution-fenced; the picker reuses
the existing modal primitive and the `RefreshDetail` server-truth path.
**Trade-offs:** the client trait grows two transition methods and a `Transition`
domain type; mocked-server tests grow transition fixtures; the TEA model gains a
typed picker overlay (loading/loaded/error), a third modal alongside compose and
the delete confirm. Field-requiring transitions are visible-but-inert until a
later slice.

## Related

- Constitution: [/constitution.md](/constitution.md) (Amendment 2)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md)
- ADR: [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md)
  (the comment-write precedent this mirrors),
  [/adr/0005-jira-client-on-gouqi-behind-trait.md](/adr/0005-jira-client-on-gouqi-behind-trait.md)
  (the client trait), [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md)
  (TEA model purity), [/adr/0024-modal-overlay-compose.md](/adr/0024-modal-overlay-compose.md)
  (the modal primitive the picker reuses)
- BDR: [/bdr/0018-status-transition-behaviors.md](/bdr/0018-status-transition-behaviors.md)
- Fork base: `active-collab-cli` status-transition parity
