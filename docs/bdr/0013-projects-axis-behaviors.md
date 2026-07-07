---
type: BDR
title: Projects axis — 'p' lists projects, Enter drills into the project's issues, back pops home
description: Observable behaviors for the browse Projects screen — 'p' from the issue list opens it, bounded navigation and click-select mirror the list, Enter/click loads the project's issues through the existing list machinery (pagination/search/detail intact), Esc pops Detail -> project list -> Projects -> mine (reloaded), failures degrade to the status line, the mine SWR snapshot is never polluted, and nothing ever exits but q.
status: Accepted
supersedes:
superseded_by:
tags: [tui, navigation, projects, behavior]
timestamp: 2026-07-07T00:00:00Z
---

# 0013. Projects axis behaviors

Behaviors for [ADR 0021](/adr/0021-projects-axis-browse.md), ported from the
fork base's BDR 0004 (screen stack) and BDR 0001 (bounded navigation),
adapted to the mine-entry + flat-screen decisions.

## Scenarios

### S1 — 'p' opens the Projects screen

- **Given** the issue list (normal mode, any origin)
- **When** the user presses `p`
- **Then** the Projects screen opens, a load is issued, and the fetched
  projects render as `KEY — name` rows with a localized header and footer
  hint; `p` in search mode types into the query, and `p` on Detail is inert.

### S2 — navigation mirrors the list

- **Given** the Projects screen with rows
- **When** the user presses `↑/↓/j/k`, wheel-scrolls, or clicks a row
- **Then** the selection moves bounded to `[0, len-1]` (click clamped to the
  last row); over-scroll/over-move/clicks never exit and never panic; an
  empty projects list makes navigation a no-op.

### S3 — drill-in loads the project's issues

- **Given** a selected project
- **When** the user presses Enter or clicks it
- **Then** the issue list screen shows that project's issues (newest-updated
  first), and everything the list already does — pagination (`n`), search
  (`/`), Enter → detail, mouse, selection — works unchanged on them.

### S4 — back pops the axis

- **Given** the stack mine → Projects → project issues → Detail
- **When** the user presses Esc or `b` repeatedly
- **Then** Detail returns to the project's issue list; the project list
  returns to the Projects screen (its rows retained, no refetch); Projects
  returns to the mine list (mine JQL restored and reloaded); Esc/`b` on the
  mine list stays a no-op; `q` quits from any screen.

### S5 — failures degrade, auth guides

- **Given** the projects fetch fails
- **When** the Projects screen is open
- **Then** a status-line error shows and the screen stays navigable (Esc
  back works); a 401 shows the standing re-auth guidance (BDR/E2); no panic,
  no exit.

### S6 — the mine snapshot stays clean

- **Given** the user drilled into a project (its issues loaded into the list)
- **When** the next `browse` starts
- **Then** the first paint still shows the mine snapshot — project-issue
  loads never write the SWR snapshot.

## Test Design

| Case | Level | Scenario | Asserts (observable) |
|---|---|---|---|
| 'p' opens + loads; inert on Detail/search | unit | S1 | screen transition + one load Cmd; no-op arms |
| Bounded nav / click clamp / empty no-op | unit | S2 | selection indices; no Quit Cmd (no-exit property extended) |
| Drill-in JQL + list reuse | unit | S3 | origin set, jql = project JQL, LoadList emitted; pagination/search arms untouched |
| Back pops | unit | S4 | screen/origin transitions incl. mine reload Cmd; mine-list Esc no-op preserved |
| Failure + 401 | unit | S5 | status error text; reauth message; screen unchanged |
| Snapshot purity | unit | S6 | no snapshot write on project ListLoaded (write seam untouched by project loads) |
| Projects render (header, rows, footer hint, pt-BR, empty state) | render (TestBackend) | S1/S2 | buffer contents under en + pt-BR |
| list_projects seam (URL, mapping, tolerance, 401) | unit (wiremock) | S1/S5 | GET /rest/api/3/project/search; ProjectRow{key,name} mapping; error classification |

## Related

- ADR: [/adr/0021-projects-axis-browse.md](/adr/0021-projects-axis-browse.md)
- BDR: [/bdr/0009-browse-mouse-interactions.md](/bdr/0009-browse-mouse-interactions.md), [/bdr/0008-browse-entry-swr-behaviors.md](/bdr/0008-browse-entry-swr-behaviors.md)
- Issues: [/issues/0043-b5a-projects-client-seam.md](/issues/0043-b5a-projects-client-seam.md), [/issues/0044-b5b-projects-screen-tui.md](/issues/0044-b5b-projects-screen-tui.md)
- Fork base: BDR 0004, BDR 0001.
