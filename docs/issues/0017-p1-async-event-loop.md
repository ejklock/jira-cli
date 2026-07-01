---
type: Issue
title: "P1 — browse TUI async event loop (EventStream + mpsc), retire block_in_place"
description: Replace the synchronous draw_loop (which blocks the draw thread on each fetch via tokio::task::block_in_place + Handle::block_on) with the async tokio::select! loop over a crossterm EventStream + an mpsc reply channel that spawns Cmd effects and feeds results back as Msg. Shell-only change (src/tui/shell.rs); the pure model.rs core and view.rs are untouched.
status: done
tracker:
tags: [tui, browse, phase2, async, refactor]
timestamp: 2026-06-30T00:00:00Z
---

# P1 — browse TUI async event loop (EventStream + mpsc)

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) open question "async result delivery",
resolved by [ADR 0008](/adr/0008-browse-tui-async-event-loop.md) (realizes
[ADR 0007](/adr/0007-browse-tui-elm-architecture.md) §2). Verified by
[BDR 0006](/bdr/0006-browse-tui-interactions.md) S9 (responsive during fetch).

## Context manifest

- **Read first:** `src/tui/shell.rs` — the whole file is the imperative shell (Humble
  Object). Today `run_tui` (L67) captures a `tokio::runtime::Handle`, and `draw_loop`
  (L143) is a synchronous loop: `terminal.draw` → `event::read()` (blocking) → `update` →
  `dispatch_cmd` (L177) which runs each fetch via
  `tokio::task::block_in_place(|| handle.block_on(fut))` (L196, L210). `fetch_and_run`
  (L42) loads the initial `mine` list before entering the loop and passes the `Handle`.
- **Do NOT touch** `src/tui/model.rs` (pure `update` + `Model`/`Msg`/`Cmd` — unchanged) or
  `src/tui/view.rs` (unchanged). The `Msg` variants needed already exist: `DetailLoaded`,
  `ListLoaded`, `LoadFailed`. `map_key_to_msg`/`map_key_in_*` (L109–141) stay as-is.
- `Cargo.toml` — crossterm is a dependency; the `event-stream` feature must be enabled for
  `crossterm::event::EventStream`. `futures` (`StreamExt`) is needed to poll the stream;
  add it if not already present (check `Cargo.toml` before adding).
- `src/main.rs` L362 calls `tui::browse(...).await`; `browse` is already `async`. The
  runtime is `#[tokio::main]` (multi-thread), so `run_tui` can become `async` and be
  awaited from `fetch_and_run` — no `Handle`/`block_on` needed.

## Approach (decided — see ADR 0008)

- Make `run_tui` an `async fn` (no captured `Handle`). `fetch_and_run` awaits it directly.
- Build a `tokio::sync::mpsc::unbounded_channel::<Msg>()`; keep the `Sender` for dispatch,
  select on the `Receiver`.
- Replace `draw_loop` with an async loop that `tokio::select!`s over:
  - a `crossterm::event::EventStream` (via `futures::StreamExt::next`) → key events mapped by
    the existing `map_key_to_msg`;
  - the mpsc `Receiver` → reply `Msg`s from completed effects.
  After each `Msg`, apply `update`, dispatch any `Cmd`s, and redraw.
- `dispatch_cmd` becomes: for `LoadDetail`/`LoadList`, `tokio::spawn` the async effect with a
  clone of the `Sender`; on completion send `DetailLoaded`/`ListLoaded` (or `LoadFailed`/
  `Back` on error) back over the channel — no `block_on`. For `OpenUrl`/`CopyToClipboard`,
  run inline (fire-and-forget) as today. `Quit` breaks the loop.
- Spawned effects need `'static` data: clone the `Instance` (and open a fresh
  `GouqiJiraClient`/`IssueCache` inside the task, as `run_search`/`load_detail` already do)
  rather than borrowing the loop's `&Instance`/`&cache`. Keep `load_detail`'s cache-or-fetch
  semantics (it may need its own `Connection`; if the cache write is hard to thread into a
  spawned task, the detail fetch may fetch-without-cache-write in the TUI path — preserve the
  cache-READ behavior and note any deviation for the Reviewer).
- Terminal raw-mode/alternate-screen setup + **unconditional teardown** on every exit path
  stays (NFR-B3). The initial `mine` fetch in `fetch_and_run` may stay as-is (pre-loop) or
  become the first spawned `LoadList` — either is acceptable; keep it simple.

## Vertical Demo

- **Given** `jira browse` against a real instance,
  **When** I press Enter on a row with a slow detail fetch,
  **Then** the `Loading…` notice is visible while the fetch runs and `q` still quits — the
  UI does not freeze.
- **And** `cargo test` stays green (pure-core + TestBackend tests unchanged) and
  `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check` are clean.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | `dispatch_cmd` no longer uses `tokio::task::block_in_place`/`Handle::block_on`; fetch effects are `tokio::spawn`ed and their results returned as `Msg` over an mpsc channel; the loop uses `tokio::select!` over a crossterm `EventStream` + the mpsc `Receiver` | inspection |
| AC2 | constraint | `src/tui/model.rs` and `src/tui/view.rs` are byte-unchanged; only `src/tui/shell.rs` (+ `Cargo.toml` for the crossterm `event-stream` feature / `futures`) changes | inspection |
| AC3 | behavior | The full test suite passes unchanged (all pure `update` + `TestBackend` tests still green); no test needed editing to accommodate the shell change | command (`cargo test`) |
| AC4 | constraint | Raw mode + alternate screen are torn down on every exit path (`q`, error); the terminal is left usable (NFR-B3) | inspection |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; no superfluous comments/banners/commented-out code | command (clippy + fmt + comment_policy) |
| AC6 | constraint | Cyclomatic ≤10 (≤8 for any new fn) / cognitive within the gate ceiling; the async loop is decomposed (guard clauses / helpers) rather than one deep function | command (complexity) |

## Out of scope

- Pagination / load-more (that is P2 + P3, [ADR 0009](/adr/0009-tui-list-pagination.md)).
- Any change to the pure `update`/`Model`/`Msg`/`Cmd` or the `view` functions.
- Mouse support, new screens, or new key bindings.

## blocked_by

(none — first slice of the P-series; retires the block_in_place shortcut)
