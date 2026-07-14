---
type: ADR
title: TTY = interactive-by-default for read commands (agent mode prints)
description: In an interactive terminal, the read commands (mine, bare, search, get, current) open the browse TUI by default; agent mode (--json, or a non-TTY stdin/stdout) keeps printing the exact human/JSON output. Routing lives in the dispatch_* layer; the *_core printers stay pure. One seeded TUI entry (TuiSeed Mine/Search/Detail) seeds the browse Model. Deliberate superset of the fork base, which routes only mine/browse to the TUI.
status: Accepted
supersedes:
superseded_by:
tags: [tui, cli, tty, agent-mode, routing, parity]
timestamp: 2026-07-14T00:00:00Z
---

# 0025. TTY = interactive-by-default for read commands

## Context

`jira-cli` exposes the same read data through two surfaces: a machine-readable
CLI (`get`, `current`, `mine`, `search`) and an interactive `browse` TUI
([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)). Today those surfaces are
bound to **separate commands**: `browse` always opens the TUI; every other read
command always prints — even in an interactive terminal.

The fork base `active-collab-cli` does **not** work that way for its list entry.
Its `dispatch_mine` computes `is_tty` and lets `mine_core` return a
`MineOutcome`: in a TTY it yields `TuiLaunch { targets }` and the shell calls
`tui::run_mine(...)`; non-TTY (or `--json`) yields `Done(code)` after printing.
So in the fork the **default in a TTY is the interactive experience**, and the
plain/JSON table is the *agent mode* (non-TTY). The fork applies this to `mine`
and bare invocation only — `get`, `current`, and `search` there still print.

Parity slice E1 was previously recorded as "no work needed" because
`bare_no_command_action` already returns `RunMine`. That captured only half the
fork's behavior: in the fork `RunMine` flows through `dispatch_mine` into
`TuiLaunch`; in `jira-cli` it flows into a static table. `jira mine` (and bare
`jira`) in a TTY therefore prints a table where the fork opens the TUI — the
inverse of the intended default.

The user's decision (2026-07-14) is to apply the TTY/agent **duality uniformly**
to all read commands, not just `mine`: an interactive terminal defaults to the
browse TUI; agent mode prints. This is a deliberate **superset** of the fork,
which routes only `mine`/`browse`.

## Decision

Make the browse TUI the **default surface for read commands in an interactive
terminal**, and reserve printing for agent mode.

1. **TTY/agent duality.** `is_tty = std::io::stdout().is_terminal() &&
   std::io::stdin().is_terminal()` (both ends interactive — matching the bare
   invocation's existing rule). A command runs in **agent mode** when
   `!is_tty` **or** `--json` is set; otherwise it runs in **interactive mode**.
   `--json` always forces agent mode, even in a TTY — the machine contract wins
   when explicitly requested.

2. **Routing lives in `dispatch_*`; the `*_core` printers stay pure.** Each of
   `dispatch_mine`, `dispatch_search`, `dispatch_get`, `dispatch_current`
   computes `is_tty` and, in interactive mode, calls the seeded TUI entry;
   otherwise it calls the existing `mine_core` / `search_core` / `get_core` /
   `current_core` **unchanged**. The cores keep their pinned stdout/stderr and
   `agent_json` contracts. Because the existing CLI tests capture output (a
   non-TTY stdout), they stay in agent mode and their contracts are untouched —
   the interactive branch is exercised by new headless routing tests.

3. **One seeded TUI entry.** `tui::browse_seeded(instance, cache, is_tty, seed,
   stderr)` with `enum TuiSeed { Mine, Search(String), Detail(String) }`, guarded
   by the existing `browse_tty_action` (its `TtyError` contract is preserved for
   the "interactive requested but not a TTY" edge). `run_tui` is generalized to
   accept an initial screen seed instead of always constructing `Screen::List`:
   - **`Mine`** = the current `fetch_and_run` path unchanged (entry SWR snapshot
     of the `mine` list per [ADR 0016](/adr/0016-swr-first-paint-browse-entry.md)
     is retained).
   - **`Search(jql)`** = fetch the JQL list via the existing `run_search` seam,
     seed `Screen::List` with `jql` set and `list_origin = Search`; **no
     snapshot** (matching ADR 0016's mine-scope-only rule).
   - **`Detail(key)`** = resolve the issue cache-or-fetch via the existing
     `fetch_issue` / detail seam, seed `Screen::Detail` with `detail =
     Some(issue)`. Back from a seeded detail exits the TUI (there is no list
     behind a single-issue entry).

4. **`current` reuses the `Detail` seed.** `dispatch_current` resolves the issue
   key from the git branch as it does today, then in interactive mode seeds
   `Detail(key)`; in agent mode it calls `current_core` unchanged.

5. **Agent mode is byte-for-byte unchanged.** `--json`, pipes, and any non-TTY
   invocation keep the exact human/JSON output and exit codes. No script or
   agent consuming `jira-cli` sees a behavior change.

## Alternatives considered

- **Port the fork's `MineOutcome`-in-core shape to all four cores.** Rejected:
  it churns four `*_core` signatures and folds a presentation/routing decision
  into the pure printers. Deciding in `dispatch_*` is a thinner seam and keeps
  the cores pure and their contracts pinned.
- **Route only `mine`/bare (strict fork parity).** Rejected per the user's
  decision to apply the duality uniformly. Recorded here as the deliberate
  divergence: `jira-cli` is a **superset** of the fork on this axis.
- **A separate TUI entry function per command.** Rejected: it duplicates the
  fetch/seed/guard plumbing. One `TuiSeed` enum behind one `browse_seeded` entry
  is the deeper module.
- **Treat a redirected stdout but interactive stdin (or vice-versa) as
  interactive.** Rejected: requiring **both** ends to be a TTY is the safe rule
  for a piped or captured invocation — it keeps `jira get KEY | less` and
  `$(jira get KEY)` in agent mode.

## Consequences

**Positive:** an interactive user gets one consistent, TUI-first read
experience across every command; the fork's split between `mine` (TUI) and
`get`/`search` (print) is gone. The change is localized to `dispatch_*` plus one
seeded entry; the cores and the pure TUI update loop are untouched.

**Accepted trade-offs:** `get`/`current`/`search` in a TTY no longer dump-and-
exit — an interactive user who wants the old print behavior adds `--json` or
pipes the output (both already the agent-mode contract). This intentionally
exceeds the fork base; the divergence is documented here rather than hidden.

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-E1 (bare/mine default), extended here to all read commands.
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md) — the TUI this routes into.
- ADR: [/adr/0016-swr-first-paint-browse-entry.md](/adr/0016-swr-first-paint-browse-entry.md) — the `Mine` seed's entry SWR, retained.
- BDR: [/bdr/0016-interactive-default-read-commands.md](/bdr/0016-interactive-default-read-commands.md) — the observable scenarios.
- Issues: [/issues/0049-e1b-list-commands-tui-default.md](/issues/0049-e1b-list-commands-tui-default.md), [/issues/0050-e1b-detail-commands-tui-default.md](/issues/0050-e1b-detail-commands-tui-default.md).
- Fork base: `active-collab-cli` `dispatch_mine` → `MineOutcome::TuiLaunch` (routes only `mine`/`browse`).
