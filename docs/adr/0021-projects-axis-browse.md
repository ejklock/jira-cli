---
type: ADR
title: Projects axis in the browse TUI — 'p' opens a Projects screen; a project drills into its issues
description: Add a Projects screen reachable via 'p' from the issue list — list projects (key + name) via a new client seam over GET /rest/api/3/project/search; Enter/click on a project loads that project's issues into the existing list machinery (JQL project = KEY), preserving pagination, search, detail, and the mine SWR snapshot. Back pops Detail → project list → Projects → mine. The fork's pushdown stack collapses to the flat Screen enum plus a ListOrigin provenance field (three fixed levels); browse entry stays the mine list.
status: Accepted
supersedes:
superseded_by:
tags: [tui, navigation, projects, client, parity]
timestamp: 2026-07-07T00:00:00Z
---

# 0021. Projects → Issues axis

## Context

[PRD 0003](/prd/0003-active-collab-parity.md) R-B5 ports the fork's project
axis. The fork (its BDR 0004) navigates **Projects → Tasks → Detail** via a
pushdown `Vec<Screen>` stack, with Projects as the entry screen. Two of its
premises don't transfer: (1) this repo's **entry is the mine list** by two
standing decisions — the bare-TTY default (PRD 0003 R-E1) and the E3 SWR
first paint whose snapshot is keyed to the `mine` scope
([ADR 0016](/adr/0016-swr-first-paint-browse-entry.md)); (2) the jira Model
keeps list state flat (rows/jql/page token on the Model, `Screen{List,
Detail}`), and Grupo B/C features hang off that shape.

## Decision

1. **Projects is key-reachable, not the entry.** `p` on the issue list
   (normal mode) opens a **Projects screen**; browse still opens on mine with
   the E3 SWR paint intact. Capability parity, jira-shaped entry.
2. **Client seam:** `JiraClient::list_projects()` — GET
   `/rest/api/3/project/search` mapped to a curated
   `ProjectRow { key, name }` list, following the `search_page` seam pattern.
   v1 fetches a single page of up to 100 projects.
3. **Flat screens + provenance instead of a stack.** `Screen` gains
   `Projects`; the Model gains `projects: Vec<ProjectRow>` + its selection,
   and `list_origin: ListOrigin { Mine, Project(key) }`. Three fixed levels
   make the fork's `Vec<Screen>` machinery unnecessary — the "stack" is fully
   determined by `(screen, list_origin)`.
4. **Drill-in reuses the list machinery wholesale.** Enter/click on a project
   sets `list_origin = Project(key)`, `jql = "project = KEY ORDER BY updated
   DESC"`, clears rows, returns to `Screen::List`, and emits the existing
   `Cmd::LoadList` — pagination (`n`), search (`/`), cards, detail, mouse,
   and selection all apply unchanged to project issues.
5. **Back semantics (pop-shaped):** Esc/`b` on Detail → the list it came
   from (unchanged); on a `Project(_)`-origin list → the Projects screen
   (projects rows retained); on Projects → the mine list (`list_origin =
   Mine`, mine JQL restored, `Cmd::LoadList` refetches — project issues
   replaced the rows, so a reload is required); on a mine-origin list →
   no-op, exactly today's behavior. `q` quits anywhere.
6. **Projects screen behaviors mirror the list:** bounded `j/k/↑/↓`, wheel,
   click selects (clamped), Enter/click drills in, nothing exits (the B1
   no-exit invariant extends); fetch failure → status-line error while
   staying navigable; 401 → the E2 re-auth message; empty list → localized
   empty state, no panic.
7. **The mine SWR snapshot is never written by project loads** — snapshot
   writes stay bound to the entry revalidation path (ADR 0016); the next
   browse entry still paints mine.
8. **No project cache in v1** — projects are fetched on open ('p'), with a
   loading status. Sliced B5a (client seam) and B5b (TUI axis).

## Alternatives considered

- **Projects as the entry screen (fork layout).** Rejected: collides with two
  standing decisions (bare-TTY mine default; mine-scoped SWR snapshot) for
  zero capability gain.
- **A real pushdown `Vec<Screen>` stack.** Rejected for now: with three fixed
  levels the provenance field yields identical observable behavior without
  refactoring the flat Model that every Grupo B feature hangs off. If Grupo C
  modals or deeper axes ever need N levels, supersede this.
- **Cache/SWR for the projects list.** Deferred: the project set changes
  rarely but the fetch is cheap and 'p' is an explicit action; the fork's
  project-name cache solved a different problem (name lookup on list rows).
- **Paginating projects beyond 100.** Deferred: recorded trade-off; the
  seam accepts a follow-up page parameter without breaking.

## Consequences

**Positive:** browse-by-project lands with the entire existing list feature
set for free; entry behavior and SWR untouched; no structural refactor.

**Accepted trade-offs:** instances with >100 projects see a truncated list
(follow-up: pagination); returning Projects → mine refetches (no snapshot
reuse); the implicit two-level "stack" must be superseded if navigation ever
deepens.

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B5.
- ADR: [/adr/0016-swr-first-paint-browse-entry.md](/adr/0016-swr-first-paint-browse-entry.md), [/adr/0017-mouse-support-browse-tui.md](/adr/0017-mouse-support-browse-tui.md), [/adr/0009-tui-list-pagination.md](/adr/0009-tui-list-pagination.md)
- BDR: [/bdr/0013-projects-axis-behaviors.md](/bdr/0013-projects-axis-behaviors.md)
- Fork base: BDR 0004 (screen stack), BDR 0001.
- Issues: [/issues/0043-b5a-projects-client-seam.md](/issues/0043-b5a-projects-client-seam.md), [/issues/0044-b5b-projects-screen-tui.md](/issues/0044-b5b-projects-screen-tui.md)
