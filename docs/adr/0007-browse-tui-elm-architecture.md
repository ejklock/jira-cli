---
type: ADR
title: "Browse TUI: a read-only Elm/TEA shell over the existing domain core (ratatui + crossterm)"
description: Build the Phase 2 interactive browse TUI as a fresh read-only Elm-style (Model/Msg/update/Cmd) application on ratatui 0.29 + crossterm 0.28, reusing the existing client/cache/i18n/ADF seams — NOT a faithful port of the active-collab-cli TUI (which carries write features that cross the read-only boundary, a Projects axis jira-cli lacks, and AC domain types). The pure update() is the functional core; the terminal loop is the imperative shell.
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, phase2, ratatui, architecture, tea, humble-object]
timestamp: 2026-06-30T00:00:00Z
---

# 0007. Browse TUI: a read-only Elm/TEA shell over the existing domain core

## Context

The [Constitution](/constitution.md) parks the interactive `browse` TUI as Phase 2,
"re-enabled as its own vertical slices after the CLI core ships" — so this needs
no constitution amendment, and the non-negotiables (single-binary, token isolation,
local-first, **pure testable core**, no drift) all still bind.

A characterization of the fork source (`../active-collab-cli/src/tui`, ~5,100 LOC)
found a well-structured **Elm/TEA** application: `Model + Msg + update + Cmd`, a pure
`update(Model, Msg) -> (Model, Vec<Cmd>)` core (~1,800 LOC, no I/O), an async shell
(`tokio::select` over a crossterm event stream + an mpsc reply channel) that spawns
`Cmd` effects, and a `ratatui 0.29 + crossterm 0.28` render layer. It has three READ
screens (Projects, Tasks, Detail) plus **WRITE** features (comment compose/edit/delete)
and an assets panel.

Two forces pull against a faithful port:

- **The write features cross the read-only boundary.** Comment compose/edit/delete
  is exactly the "writing to Jira" the constitution defers behind its own ADR. A
  faithful port would import a constitution violation.
- **AC-shaped cruft.** The Projects screen (jira-cli is issue-centric, with no
  project-browse axis), the assets/attachments panel, and the AC domain types
  (`Task`/`Project`/`Comment`) do not map onto jira-cli's read model
  (`Issue`/`IssueRow`).

What *is* valuable and reusable is the **architecture** (Elm/TEA + the pure-core
testability it buys) and the **rendering approach** (ratatui), not the AC code.

## Decision

Build the browse TUI as a **fresh, read-only Elm/TEA application** — adopting the
AC architecture and stack, re-implementing only the read subset against jira-cli's
existing seams. Not a file-level port.

1. **Stack.** `ratatui 0.29` (render) + `crossterm 0.28` (terminal + event stream),
   driven on the existing `tokio` runtime. No other TUI crates.
2. **Elm/TEA shape, Functional Core + Imperative Shell.**
   - **Functional core:** a pure `update(model, msg) -> (Model, Vec<Cmd>)` plus the
     `Model`/`Screen`/`Msg`/`Cmd` types. No terminal, no network, no clock. This is
     where ALL navigation/scroll/selection/search-input logic lives and is
     unit-tested directly (the constitution's "pure, testable core"; Humble Object).
   - **Imperative shell:** raw-mode setup/teardown, the `crossterm` event loop, the
     `ratatui` draw call, and `dispatch_cmds` spawning async effects whose results
     return as new `Msg` over an mpsc channel. The shell is kept thin and is NOT
     unit-tested (validated by the manual demo gate).
3. **Reuse the DATA seams, not the render commands.** `Cmd` handlers call the same
   data path the CLI uses — `JiraClient::search(jql, limit)` for the list and the
   cache-or-fetch issue load (`commands`' `load_issue`/`fetch_and_cache`) for detail
   — returning domain types (`IssueRow`, `Issue`). The TUI renders those types with
   ratatui. It does **not** call the rendering `*_core` functions (`mine_core`,
   `search_core`, `get_core`) — those are the CLI's imperative shell, which write to
   a `Write`. Both shells sit over the **same** models / client / store / i18n / ADF
   flatten, so the "no drift" non-negotiable holds: there is one domain core, two
   shells (CLI and TUI).
4. **Read-only scope.** Screens: **issue list** (mine, then JQL search), **issue
   detail** (summary/status/description via ADF flatten/comments, scroll), plus
   read affordances (open the issue URL in the browser, copy the key to the
   clipboard). NO comment write, NO Projects screen, NO assets panel. Any write
   stays out until its own constitution amendment + ADR.
5. **Testing instrument (no browser-gate).** A terminal TUI is not a web surface, so
   the `verify_by: browser` gate does not apply. The deterministic render instrument
   is **ratatui's `TestBackend`**: render `view(&model)` into an in-memory buffer and
   assert cell content (e.g. the list shows my issue keys, the detail shows the
   summary). Combined with pure `update` unit tests, this covers behavior without a
   real terminal. The live "open it and watch it work" demo is the second gate.
6. **Module layout, grown not front-loaded.** Start as a single `src/tui.rs`
   (`#[path]` test module, matching the project's existing modules), split into a
   `src/tui/` submodule (`model.rs` pure / shell / `view.rs`) when it earns the split
   — recorded as a refactor slice, with the architecture diagram updated in the same
   change (maintenance invariant).

## Alternatives considered

- **Faithful port of the AC TUI.** Rejected: imports write features that violate the
  read-only constitution, drags the Projects/assets cruft and AC domain types, and is
  ~5,100 LOC to adapt. The architecture is worth adopting; the code is not.
- **An immediate-mode / non-TEA loop** (mutable state mutated in place in the draw
  loop). Rejected: tangles state + I/O + render, defeating the pure-core
  non-negotiable and making the logic untestable without a terminal.
- **A different toolkit (cursive, termion).** Rejected: ratatui 0.29 + crossterm is
  the AC stack (proven for this exact shape), actively maintained, and pairs with the
  existing tokio runtime; no reason to diverge.

## Consequences

**Positive:**

- The interactive browser reuses the proven TEA architecture and the existing,
  tested domain core/client/cache — net-new code is the read-only UI only.
- All UI logic is a pure function, unit- and mutation-tested off the terminal; the
  shell is a thin Humble Object.
- Read-only by construction → no constitution amendment, no write surface to secure.

**Accepted trade-offs:**

- Two new dependencies (`ratatui`, `crossterm`) and a second imperative shell over
  the domain core. Justified: the shells share one core, so no contract drift.
- Re-implementing the read screens rather than copying them is more up-front work
  than a port, but avoids the write-boundary violation and the AC coupling.
- The terminal shell (raw mode, event loop, draw) is not unit-tested; it rests on the
  manual demo gate + the `TestBackend` view tests for the rendered output.

## Related

- Constitution: [/constitution.md](/constitution.md) (Phase 2: browse TUI)
- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- ADR: [/adr/0005-jira-client-on-gouqi-behind-trait.md](/adr/0005-jira-client-on-gouqi-behind-trait.md)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md)
- BDR: [/bdr/0006-browse-tui-interactions.md](/bdr/0006-browse-tui-interactions.md)
