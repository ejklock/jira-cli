---
type: BDR
title: "read commands open the browse TUI in an interactive terminal; agent mode prints"
description: In an interactive terminal (stdout AND stdin are TTYs) and without --json, the read commands open the browse TUI seeded to the right screen — mine/bare and search seed the list, get/current seed the issue detail. With --json, a redirected/piped stdout or stdin, or any non-TTY invocation, the exact human/JSON output and exit codes are unchanged (agent mode). The interactive-requested-but-not-a-TTY edge keeps the existing browse TtyError.
status: Accepted
superseded_by:
supersedes:
tags: [tui, cli, tty, agent-mode, routing, parity]
timestamp: 2026-07-14T00:00:00Z
---

# 0016. Read commands interactive-by-default

## Context

The routing decision from [ADR 0025](/adr/0025-tty-interactive-default-read-commands.md):
an interactive terminal defaults to the browse TUI for every read command;
agent mode (`--json` or a non-TTY end) prints. This BDR pins the observable
behavior so a script or agent consuming `jira-cli` sees **no** change while an
interactive user gets a uniform TUI-first experience.

## Textual Description

For each read command (`mine`, bare `jira`, `search`, `get`, `current`):

- **Interactive mode** = `stdout` AND `stdin` are both TTYs **and** `--json`
  is absent. The command opens the browse TUI seeded to:
  - `mine` and bare `jira` → the **mine list** (existing entry, with its
    entry-SWR snapshot).
  - `search <jql>` → the **list** of that JQL's results.
  - `get <key>` and `current` → the **detail** of the resolved issue (the
    branch's key for `current`).
- **Agent mode** = `--json` is set, **or** either `stdout`/`stdin` is not a
  TTY (piped, redirected, captured, or run by an agent). The command prints the
  exact human or JSON output and exit code it prints today — byte-for-byte.
- The **interactive-requested-but-not-a-TTY** edge keeps the existing browse
  behavior: the `TtyError` message on stderr, exit 1. (This is reachable only
  through the seeded entry's guard, not through normal agent-mode printing.)
- Interactive routing performs **no** write and changes no data — it only
  chooses the presentation surface.

## Scenarios

**Scenario 1: `mine` in a terminal opens the TUI** — Given a configured
instance and an interactive terminal, When the user runs `jira mine`, Then the
browse TUI opens seeded to the mine list (not a printed table).

**Scenario 2: bare `jira` in a terminal opens the TUI** — Given an interactive
terminal, When the user runs `jira` with no subcommand, Then it routes to the
mine list in the browse TUI.

**Scenario 3: `mine --json` prints** — Given an interactive terminal, When the
user runs `jira mine --json`, Then the exact minified JSON list prints and the
TUI does not open, exit 0.

**Scenario 4: piped `mine` prints** — Given `jira mine | cat` (stdout not a
TTY), When it runs, Then the plain table prints unchanged, no TUI.

**Scenario 5: `search` in a terminal opens the list** — Given an interactive
terminal, When the user runs `jira search "project = NIKE"`, Then the browse
TUI opens seeded to that JQL's result list.

**Scenario 6: `search --json` / piped prints** — Given `--json` or a non-TTY,
When the user runs the same search, Then the exact JSON/human search output
prints, no TUI.

**Scenario 7: `get` in a terminal opens the detail** — Given an interactive
terminal, When the user runs `jira get NIKE-640`, Then the browse TUI opens
seeded directly to that issue's detail screen.

**Scenario 8: `get --json` / piped prints** — Given `--json` or a non-TTY,
When the user runs `jira get NIKE-640`, Then the exact agent_json / human
render prints, no TUI, unchanged from today.

**Scenario 9: `current` in a terminal opens the branch issue's detail** —
Given a git branch carrying an issue key and an interactive terminal, When the
user runs `jira current`, Then the browse TUI opens seeded to that issue's
detail; with `--json` or a non-TTY it prints unchanged.

**Scenario 10: interactive requested but not a TTY** — Given the seeded entry
is reached without a TTY, Then the existing `TtyError` prints on stderr, exit
1 (the browse contract is preserved, not a new code path).

## Test Design

The routing decision is a **pure** function of `(is_tty, json)` →
`Surface { Interactive, Agent }`, unit-tested headless with no terminal. Each
`dispatch_*` is asserted to call the seeded TUI entry with the correct
`TuiSeed` in interactive mode and the unchanged `*_core` printer in agent mode
(the cores' output contracts are re-used, not re-proven here — they are pinned
by the existing `get`/`mine`/`search`/`current` tests, which run non-TTY and
therefore stay in agent mode). The TUI seeding itself (`Screen::List` vs
`Screen::Detail` from a `TuiSeed`) is asserted on the constructed `Model`.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Surface: TTY + no json | unit | 1,5,7 | `command_surface(true,false) == Interactive` | interactive default |
| Surface: TTY + json | unit | 3,6,8 | `command_surface(true,true) == Agent` | --json forces agent |
| Surface: non-TTY | unit | 4 | `command_surface(false,false) == Agent` | pipe/capture prints |
| mine/bare seed | unit | 1,2 | interactive `mine`/bare builds `TuiSeed::Mine`; Model on `Screen::List`, mine jql | list seed |
| mine agent path | unit | 3,4 | json/non-TTY calls `mine_core`; TUI not constructed | no regression |
| search seed | unit | 5 | interactive `search` builds `TuiSeed::Search(jql)`; Model list seeded with that jql, no snapshot | search list seed |
| search agent path | unit | 6 | json/non-TTY calls `search_core` unchanged | no regression |
| get seed | unit | 7 | interactive `get` builds `TuiSeed::Detail(key)`; Model on `Screen::Detail`, `detail=Some` | detail seed |
| get/current agent path | unit | 8,9 | json/non-TTY calls `get_core`/`current_core` unchanged | no regression |
| current branch seed | unit | 9 | interactive `current` resolves branch key into `TuiSeed::Detail(key)` | branch → detail |
| not a TTY at seeded entry | unit | 10 | `browse_tty_action(false) == TtyError`; TtyError on stderr, exit 1 | guard preserved |

## Related

- ADR: [/adr/0025-tty-interactive-default-read-commands.md](/adr/0025-tty-interactive-default-read-commands.md)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md), [/adr/0016-swr-first-paint-browse-entry.md](/adr/0016-swr-first-paint-browse-entry.md)
- Issues: [/issues/0049-e1b-list-commands-tui-default.md](/issues/0049-e1b-list-commands-tui-default.md), [/issues/0050-e1b-detail-commands-tui-default.md](/issues/0050-e1b-detail-commands-tui-default.md)
- Fork base: `active-collab-cli` `MineOutcome::TuiLaunch` (mine/browse only)
