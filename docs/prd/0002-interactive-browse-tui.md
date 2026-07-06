---
type: PRD
title: "Interactive browse TUI (Phase 2, read-only)"
description: A read-only full-screen terminal UI (`jira browse`) to navigate issues interactively — list (mine/JQL), open detail, search, and open-link/copy-key — over the existing local-first read core.
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, phase2, read]
timestamp: 2026-06-30T00:00:00Z
---

# 0002. Interactive browse TUI (Phase 2, read-only)

## Problem

The v1 CLI ([PRD 0001](/prd/0001-jira-cloud-read-cli.md)) reads issues one command
at a time. When a developer wants to scan their assigned issues, open one to read
its detail, then jump to another or run a query, the per-command round-trips are
clumsy. An interactive full-screen browser keeps the flow in one place — list,
open, read, search — without leaving the terminal. The fork source already proved
this shape; jira-cli re-enables it read-only as its own Phase 2 slices.

## Goals

1. Launch a full-screen interactive browser with `jira browse` and navigate issues
   with the keyboard.
2. Read issue detail (summary, status, description, comments) inside the browser.
3. Run JQL interactively and repopulate the list without restarting.
4. Reach the underlying issue fast: open its URL in the browser, copy its key.

## Non-goals

> **Superseded (2026-07-06):** the projects-axis, attachments-panel, and mouse
> non-goals below were reopened by [PRD 0003](/prd/0003-active-collab-parity.md)
> (total-parity program); the write non-goal was narrowed by Constitution
> Amendment 1 + [ADR 0015](/adr/0015-comment-write-enablement.md). Kept verbatim
> for the record.

- **Any write** (comment, transition, worklog) — stays out behind the constitution's
  write boundary and its own future ADR. The TUI is read-only ([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)).
- **A Projects browse axis** and **an assets/attachments panel** — AC-fork features
  that do not map onto jira-cli's issue-centric read model.
- **Mouse-first interaction** — keyboard is the contract; mouse support (click/scroll)
  is a possible later slice, not required for v1 of browse.
- **Jira Server/DC, encryption, native binaries** — unchanged Phase 2 items, separate.

## Requirements

Each requirement is delivered by the BDR scenario(s) and issue slice(s) it links;
all behavior is read-only and reuses the existing client/cache/i18n/ADF seams
([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)).

- **R1 — Launch + list.** `jira browse` enters a full-screen TUI showing my open
  issues (the `mine` JQL), navigable with ↑/↓, quit with `q`; a non-TTY invocation
  errors and exits non-zero. Verified by [BDR 0006](/bdr/0006-browse-tui-interactions.md) S1/S2.
- **R2 — Open detail.** Selecting a row (Enter) opens a detail view (summary, status,
  type, assignee, description via ADF flatten, comments), scrollable; Esc/`b` returns
  to the list. Verified by [BDR 0006](/bdr/0006-browse-tui-interactions.md) S3.
- **R3 — Interactive search.** A search input runs arbitrary JQL and repopulates the
  list in place; an invalid JQL shows the error without crashing the UI. Verified by
  [BDR 0006](/bdr/0006-browse-tui-interactions.md) S4/S5.
- **R4 — Read affordances.** From the list/detail, open the selected issue's URL in
  the system browser and copy its key to the clipboard. Verified by
  [BDR 0006](/bdr/0006-browse-tui-interactions.md) S6.

## Acceptance

The browse capability is acceptable when, against a real Jira Cloud instance in a
real terminal:

1. `jira browse` opens a full-screen list of my open issues; ↑/↓ moves the
   selection; `q` exits cleanly and the terminal is restored (no broken raw mode).
2. `echo | jira browse` (non-TTY) prints the "requires an interactive terminal"
   error and exits non-zero, touching no network.
3. Enter on a row opens its detail (summary/status/description/comments); Esc returns
   to the list with the same selection.
4. Typing a JQL query and submitting repopulates the list; an invalid JQL shows an
   inline error and the UI stays usable.
5. The open-link affordance launches the issue's `/browse/KEY` URL; copy puts the key
   on the clipboard.

## Quality-attribute scenarios (NFR, instrument-bound)

| ID | Scenario | Measure | Instrument |
|---|---|---|---|
| NFR-B1 Pure UI core | Any navigation/scroll/search-input/selection logic | `update(model, msg)` is pure (no I/O) and decides the next state | unit tests on `update` |
| NFR-B2 Deterministic render | Any screen | `view(model)` rendered to a buffer shows the expected cells | ratatui `TestBackend` unit test |
| NFR-B3 Terminal restored | `q`/panic/normal exit | raw mode + alternate screen are torn down; terminal usable after | manual demo gate (shell is Humble Object) |
| NFR-B4 Read-only | Whole TUI | no request mutates Jira; only GET/search are issued | code review + absence of write Cmds |
| NFR-B5 Token isolation | Any TUI fetch | reuses the host-pinned client; no `Authorization` off-host | inherited NFR-1 client test |

## Open questions (resolved)

- **Async result delivery while drawing** — **RESOLVED** by
  [ADR 0008](/adr/0008-browse-tui-async-event-loop.md). The B0–B4 shell loaded fetches
  synchronously via `block_in_place` (documented interim); the async `tokio::select!` loop
  over a crossterm `EventStream` + an mpsc reply channel (ADR 0007 §2's intended shell) now
  spawns each `Cmd` effect and feeds its result back as a `Msg`, so the UI stays responsive
  during I/O (`Loading…` visible, `q` honored). Verified by [BDR 0006](/bdr/0006-browse-tui-interactions.md) S9.
- **List paging in the TUI** — **RESOLVED** by
  [ADR 0009](/adr/0009-tui-list-pagination.md). gouqi already exposes the V3 `nextPageToken`;
  the domain `SearchResult` now carries it and `JiraClient::search_page` fetches subsequent
  pages, which the browse list **appends** on an explicit load-more action (bounded,
  read-only, one page per action). The CLI first-page + `--limit` contract is unchanged.
  Verified by [BDR 0006](/bdr/0006-browse-tui-interactions.md) S8.

## References

- Constitution: [/constitution.md](/constitution.md)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md)
- BDR: [/bdr/0006-browse-tui-interactions.md](/bdr/0006-browse-tui-interactions.md)
- PRD: [/prd/0001-jira-cloud-read-cli.md](/prd/0001-jira-cloud-read-cli.md)
