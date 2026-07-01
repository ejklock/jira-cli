---
type: ADR
title: "Browse TUI: realize the async select event loop (EventStream + mpsc), retiring the block_in_place shell"
description: Replace the interim synchronous draw_loop (which ran each fetch Cmd via tokio::task::block_in_place + Handle::block_on, freezing the UI during I/O) with the async shell ADR 0007 §2 already prescribed — a tokio::select! over a crossterm EventStream and an mpsc reply channel that spawns Cmd effects and feeds their results back as Msg. The pure update() core and view are unchanged.
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, phase2, tea, async, humble-object]
timestamp: 2026-06-30T00:00:00Z
---

# 0008. Browse TUI: realize the async select event loop (EventStream + mpsc)

## Context

[ADR 0007](/adr/0007-browse-tui-elm-architecture.md) §2 specified the browse TUI's
imperative shell as an **async loop** — "a `tokio::select` over a crossterm event stream +
an mpsc reply channel that spawns `Cmd` effects whose results return as new `Msg`". The
B0–B4 slices, however, took the documented shortcut (§Open questions of
[PRD 0002](/prd/0002-interactive-browse-tui.md): "the first slice may load synchronously
before the loop and defer the async-during-loop refresh to a named slice"): the shell in
`src/tui/shell.rs` runs a synchronous `draw_loop` that, on each fetch `Cmd`
(`LoadDetail`/`LoadList`), calls `tokio::task::block_in_place(|| handle.block_on(fut))`.

The consequence is user-visible: while a fetch runs, the draw thread is **blocked**, so
the UI freezes on the last painted frame — the `Loading…` notice is never shown (there is
no repaint between the `update` that sets `detail=None` and the blocking fetch), and the
user cannot `q`/cancel until the request returns. This is the interim state PRD 0002's open
question named; this ADR retires it.

## Decision

Replace the synchronous `draw_loop` + `block_in_place` dispatch with the async shell
ADR 0007 §2 intended. **The pure `update(model, msg) -> (Model, Vec<Cmd>)` core and the
`view`/`view_*` functions are unchanged** — this is a shell-only change (Humble Object),
validated by the manual demo gate plus the unchanged pure-core unit/`TestBackend` tests.

1. **Async loop with `tokio::select!`.** `run_tui` becomes an `async fn` driven on the
   existing multi-thread tokio runtime (no more captured `Handle` + `block_on`). The loop
   selects over two sources:
   - a `crossterm::event::EventStream` (crossterm `event-stream` feature + `futures::StreamExt`)
     yielding terminal events → mapped to `Msg` exactly as today (`map_key_to_msg` unchanged);
   - an `mpsc::UnboundedReceiver<Msg>` carrying the results of spawned effects.
2. **Cmd effects are spawned, not blocked.** `dispatch_cmd` `tokio::spawn`s each async
   effect (detail/list fetch); when it resolves it sends the result `Msg`
   (`DetailLoaded`/`ListLoaded`/`LoadFailed`) back on the mpsc `Sender`. The loop keeps
   drawing between dispatch and completion, so `Loading…` is visible and the UI stays
   responsive (a keypress — including `q` — is still processed while a fetch is in flight).
3. **Redraw on every loop turn.** After each selected event (key or reply `Msg`), the loop
   applies `update` and repaints. No busy-poll: `select!` awaits, so an idle UI costs nothing.
4. **Cheap synchronous effects stay inline.** `OpenUrl`/`CopyToClipboard` (fire-and-forget
   `std::process::Command`) do not need spawning; they run inline in the dispatch arm and
   emit no reply `Msg`, as today.
5. **Terminal teardown is unconditional.** Raw mode + alternate screen are still torn down
   on every exit path (`q`, error, and — via the shell's structure — a panic-safe teardown),
   preserving [PRD 0002](/prd/0002-interactive-browse-tui.md) NFR-B3.

## Alternatives considered

- **Keep `block_in_place`, add a pre-fetch `Loading…` repaint.** Rejected: paints the
  loading frame once but the UI still freezes for the whole request (no cancel, no `q`),
  so it does not deliver the responsiveness the open question is about — it only masks it.
- **A second OS thread for input + channel to the draw thread.** Rejected: reintroduces
  manual thread/sync plumbing the tokio runtime already provides; `tokio::select!` over an
  `EventStream` is the idiomatic, lower-surface path and is exactly what ADR 0007 named.
- **An actor/framework (e.g. a TUI framework's own runtime).** Rejected: the app already
  owns a tokio runtime; adding a second scheduler is unjustified for a read-only browser.

## Consequences

**Positive:**

- The UI is responsive during fetches: `Loading…` shows, keys (incl. `q`) are honored,
  and a slow request never freezes the terminal. This is the behavior ADR 0007 intended.
- The change is confined to the shell (`src/tui/shell.rs`): the pure core and view are
  untouched, so every existing `update`/`TestBackend` test stays green unchanged, and the
  functional-core/Humble-Object boundary is preserved (NFR-B1/B2).
- Unblocks the pagination slice ([ADR 0009](/adr/0009-tui-list-pagination.md)): a
  `LoadMore` effect is naturally another spawned Cmd on the same channel.

**Accepted trade-offs:**

- Two dependency surfaces: the crossterm `event-stream` feature and `futures`
  (`StreamExt`). Both are already in the ratatui/crossterm ecosystem the app depends on;
  no new unrelated crate family.
- The shell grows a `Sender<Msg>`/`Receiver<Msg>` and an `async` signature; it remains a
  thin, non-unit-tested Humble Object validated by the manual demo gate (NFR-B3). Ordering
  of concurrently-returning effects is not guaranteed, which is acceptable for a read-only
  browser (the latest `ListLoaded`/`DetailLoaded` wins; no write races exist).

## Related

- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md) (§2 async shell — this realizes it)
- ADR: [/adr/0009-tui-list-pagination.md](/adr/0009-tui-list-pagination.md) (LoadMore rides this loop)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md) (open question: async result delivery)
- BDR: [/bdr/0006-browse-tui-interactions.md](/bdr/0006-browse-tui-interactions.md)
