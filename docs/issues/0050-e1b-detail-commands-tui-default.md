---
type: Issue
title: "E1b — single-issue read commands open the detail TUI in a terminal (get, current)"
description: In an interactive terminal (stdout AND stdin TTY) without --json, `jira get <key>` and `jira current` open the browse TUI seeded directly to the issue detail; --json / non-TTY keep printing unchanged. Extends tui::browse_seeded with TuiSeed::Detail(key) (cache-or-fetch the issue, seed Screen::Detail with detail=Some(issue)); current resolves the branch key first and reuses the same seed.
status: done
labels: [tui, cli, tty, agent-mode, routing, parity]
blocked_by: 0049
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## E1b — detail commands interactive-by-default

Implements [ADR 0025](/adr/0025-tty-interactive-default-read-commands.md),
behaviors [BDR 0016](/bdr/0016-interactive-default-read-commands.md) S7–S9.
Builds on issue [0049](/issues/0049-e1b-list-commands-tui-default.md) (the
`browse_seeded` entry, `command_surface` decision, and seeded `run_tui`).

Delivered in two code slices:

- **S3 — get → TUI detail.** Extend `browse_seeded` with `TuiSeed::Detail(key)`
  (resolve cache-or-fetch via the existing `fetch_issue`/detail seam, seed
  `Screen::Detail` with `detail = Some(issue)`; back exits the TUI).
  `dispatch_get` routes interactive → `browse_seeded(Detail(key))`, else
  `get_core`.
- **S4 — current → TUI detail from the branch.** `dispatch_current` resolves the
  issue key from the git branch as today, then routes interactive →
  `browse_seeded(Detail(key))`, else `current_core` (reuses S3's detail seed).

Scope: `src/main.rs` (dispatch_get / dispatch_current routing), `src/tui/shell.rs`
(`TuiSeed::Detail` seed + detail-screen `run_tui`), `src/tui/model.rs` (detail
seed on `Screen::Detail`), `tests/unit/...`. Agent-mode output contracts are
unchanged (existing non-TTY tests stay green); new headless tests cover the
routing decision and the detail-seeded `Model`.
