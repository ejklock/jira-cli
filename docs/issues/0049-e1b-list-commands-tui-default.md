---
type: Issue
title: "E1b — list read commands open the browse TUI in a terminal (mine, bare, search)"
description: In an interactive terminal (stdout AND stdin TTY) without --json, `jira mine`, bare `jira`, and `jira search <jql>` open the browse TUI seeded to the list; --json / non-TTY keep printing unchanged. Routing lives in dispatch_*; a single seeded entry tui::browse_seeded(TuiSeed{Mine,Search}) seeds the Model on Screen::List. mine reuses the existing entry SWR snapshot; search seeds the JQL list with no snapshot.
status: done
labels: [tui, cli, tty, agent-mode, routing, parity]
blocked_by:
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## E1b — list commands interactive-by-default

Implements [ADR 0025](/adr/0025-tty-interactive-default-read-commands.md),
behaviors [BDR 0016](/bdr/0016-interactive-default-read-commands.md) S1–S6, S10.
Extends [PRD 0003](/prd/0003-active-collab-parity.md) R-E1 from bare/mine to the
list read commands. Deliberate superset of the fork (which routes only
mine/browse).

Delivered in two code slices:

- **S1 — mine + bare → TUI.** Introduce `enum TuiSeed { Mine, Search(String),
  Detail(String) }` and `tui::browse_seeded(instance, cache, is_tty, seed,
  stderr)`; route the existing `browse` through `TuiSeed::Mine`. Add the pure
  `command_surface(is_tty, json) -> Surface` decision. `dispatch_mine` and the
  bare `RunMine` path call `browse_seeded(Mine)` in interactive mode, else
  `mine_core`. `is_tty = stdout().is_terminal() && stdin().is_terminal()`.
- **S2 — search → TUI list.** Extend `browse_seeded` with `TuiSeed::Search(jql)`
  (fetch via `run_search`, seed `Screen::List` with the jql, `list_origin =
  Search`, no snapshot); generalize `run_tui` to accept an initial-screen seed.
  `dispatch_search` routes interactive → `browse_seeded(Search(jql))`, else
  `search_core`.

Scope: `src/main.rs` (dispatch routing + bare path), `src/tui/shell.rs`
(`browse_seeded`, seeded `run_tui`), `src/tui/model.rs` (list seed fields),
`src/cli.rs` (`command_surface`, `Surface`, `TuiSeed` if placed here),
`tests/unit/...`. Agent-mode output contracts are unchanged (existing
non-TTY tests stay green); new headless tests cover the routing decision and
the seeded `Model`.
