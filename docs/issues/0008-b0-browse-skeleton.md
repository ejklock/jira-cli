---
type: Issue
title: "B0 — browse TUI walking skeleton: command + raw-mode shell + quit"
description: Wire `jira browse`, add ratatui + crossterm, enter/exit the alternate screen + raw mode, draw a placeholder frame, quit on q, and guard non-TTY. The tracer bullet that proves the terminal lifecycle before any data screen.
status: open
tracker:
tags: [tui, browse, phase2, skeleton]
timestamp: 2026-06-30T00:00:00Z
---

# B0 — browse TUI walking skeleton: command + raw-mode shell + quit

## Objective link

North Star: [Constitution](/constitution.md) (Phase 2 browse TUI) → [PRD 0002](/prd/0002-interactive-browse-tui.md)
R1 (launch + TTY guard) → [BDR 0006](/bdr/0006-browse-tui-interactions.md) S2 →
architecture [ADR 0007](/adr/0007-browse-tui-elm-architecture.md). This is slice 0
(the walking skeleton): the thinnest end-to-end path that runs and is demoable.

## Context manifest

- **Read first:** `src/cli.rs` (`KNOWN_COMMANDS` L5, the `Command` enum L15-28,
  `normalize_argv` L140), `src/main.rs` (mod list L1-10, `dispatch` L81-89, the
  `is_terminal` use L41), `locales/pt_BR.json:73` (the inherited
  `Error: 'browse' requires an interactive terminal (TTY).` key — reuse it).
- **Add:** `ratatui = "0.29"` and `crossterm = "0.28"` to `Cargo.toml` (per
  [ADR 0007](/adr/0007-browse-tui-elm-architecture.md)).
- **Create `src/tui.rs`** (single file for now, `#[cfg(test)] #[path = "../tests/unit/tui.rs"] mod tests;`
  per the project pattern): a `pub async fn browse(...) -> i32` entry that (1) if
  stdout is not a TTY, prints the TTY-error chrome via the i18n seam and returns a
  non-zero code WITHOUT touching the network; (2) otherwise enters the alternate
  screen + raw mode, draws a single placeholder frame (e.g. a bordered block titled
  "browse" with a footer hint), reads crossterm key events in a loop, and on `q`
  (or Ctrl+C) restores the terminal (leave raw mode + alternate screen) and returns 0.
- **Wire the command:** add `"browse"` to `KNOWN_COMMANDS`; add a `Browse(BrowseArgs)`
  variant to `Command` with an `--instance` option (mirror `MineArgs`'s instance
  field, no json/limit); route it in `main.rs` `dispatch` to a `dispatch_browse` that
  resolves the instance (reuse `resolve_single_instance`) and calls `tui::browse`.
  `init_language()` runs before dispatch as for the other commands.
- **Pattern:** the imperative shell is thin and not unit-tested (Humble Object); the
  pure logic to unit-test in this slice is the TTY-routing decision — extract a pure
  helper (e.g. `fn browse_tty_action(is_tty: bool) -> BrowseAction` mirroring
  `bare_no_command_action`) so the guard is testable without a terminal.

## Vertical Demo

- **Given** a real terminal and a configured instance,
  **When** I run `jira browse`,
  **Then** a full-screen placeholder UI appears; pressing `q` exits cleanly and the
  shell prompt is back to normal (raw mode fully restored, no garbled terminal).
- **Unhappy path (the instrumented one):** **Given** stdout is not a TTY,
  **When** I run `echo | jira browse`,
  **Then** it prints `Error: 'browse' requires an interactive terminal (TTY).` and
  exits non-zero, making no network request.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | The pure TTY-routing helper returns "run the TUI" when is_tty=true and "TTY error / non-zero" when is_tty=false | test |
| AC2 | behavior | Non-TTY `browse` prints the inherited TTY-error chrome (via i18n) and returns a non-zero exit code without constructing a client or issuing any request | test |
| AC3 | constraint | `browse` is wired: in `KNOWN_COMMANDS`, as a `Command::Browse` variant, and routed in `main.rs` dispatch; `cargo build` + clippy --all-targets clean | command |
| AC4 | constraint | No superfluous comments / banners / commented-out code; only non-obvious why-comments | inspection + comment_policy |
| AC5 | constraint | Cyclomatic ≤ 10 (≤ 8 for new fns) / cognitive within the gate ceiling | quality-gate complexity |
| AC6 | constraint | Honors ADR 0007: pure routing helper is testable off-terminal; the raw-mode loop is the only untested shell; no write Cmd exists | inspection (Reviewer) |

## Out of scope

- The actual issue **list** (data fetch + navigable rows) — slice B1 (issue 0009).
- Issue **detail**, **interactive search**, **open-link/copy** — slices B2–B4.
- Splitting `src/tui.rs` into a `src/tui/` submodule — a later refactor slice when it
  earns the split (ADR 0007).
- Async result delivery during the loop (channel plumbing) — deferred (PRD 0002 open
  question); this slice only needs a synchronous draw + key loop.

## blocked_by

[0001](/issues/0001-j0-skeleton-setup-get.md) (the CLI core this builds on)
