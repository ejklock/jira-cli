---
type: Issue
title: "B5b — Projects screen in the browse TUI ('p' opens, Enter drills into the project's issues)"
description: Screen::Projects + ListOrigin provenance; 'p' from the issue list opens it (loading status), bounded nav/wheel/click mirror the list, Enter/click sets the project JQL and reuses the whole list machinery, Esc pops Detail -> project list -> Projects -> mine (reloaded), failures/401 degrade to the status line, mine SWR snapshot never written by project loads.
status: open
labels: [tui, navigation, projects, parity]
blocked_by: 0043
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## B5b — Projects screen + axis navigation

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B5 (TUI half) per
[ADR 0021](/adr/0021-projects-axis-browse.md), behaviors
[BDR 0013](/bdr/0013-projects-axis-behaviors.md) S1–S6.
