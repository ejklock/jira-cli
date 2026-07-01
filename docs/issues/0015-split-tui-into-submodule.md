---
type: Issue
title: "Split src/tui.rs into a src/tui/ submodule (model / view / shell)"
description: src/tui.rs has grown past 700 lines mixing the pure functional core (Model/Msg/Cmd/update), the ratatui view, and the imperative terminal shell. ADR 0007 §6 prescribes splitting into a src/tui/ submodule once it earns the split. This is a pure mechanical move — no logic change — into model.rs (pure) / view.rs / shell.rs, with the architecture diagram updated in the same change (maintenance invariant).
status: done
tracker:
tags: [tui, refactor, module-growth, debt]
timestamp: 2026-06-30T00:00:00Z
---

# Split src/tui.rs into a src/tui/ submodule

## Objective link

[ADR 0007](/adr/0007-browse-tui-elm-architecture.md) §6 ("Module layout, grown not
front-loaded"): start as a single `src/tui.rs`, "split into a `src/tui/` submodule
(`model.rs` pure / shell / `view.rs`) when it earns the split — recorded as a refactor
slice, with the architecture diagram updated in the same change (maintenance invariant)."
The file is now ~700 lines spanning all three concerns; the split is earned. This closes
the first deferred TUI debt item recorded in [issues/index.md](/issues/index.md).

## Context manifest

- **Read first:** `src/tui.rs` (the whole file). Three concern bands already exist and
  map cleanly onto the target submodules — the split is a move, not a redesign:
  - **Pure functional core → `src/tui/model.rs`:** `Screen`, `Model`, `Msg`, `Cmd`
    (L27–69) and `update` + every `update_*` helper (L71–249). Depends only on
    `crate::models::{Issue, IssueRow}` and `crate::render::issue_browse_url`. **No
    crossterm, no ratatui, no tokio, no io** — keep it that way (ADR 0007 §2 "No
    terminal, no network, no clock").
  - **View → `src/tui/view.rs`:** `view`, `view_list`, `view_detail` (L509–698) plus
    the render consts `LOADING_NOTICE`, `SEARCH_PROMPT`, `SEARCH_ERROR_PREFIX`
    (L23–25). Depends on `ratatui`, `crate::i18n::t`, and `model` types.
  - **Imperative shell (Humble Object) → `src/tui/shell.rs`:** `browse`,
    `fetch_and_run`, `run_tui`, `draw_loop`, `dispatch_cmd`, `load_detail`,
    `run_search`, `spawn_opener`, `copy_to_clipboard`, and the key-mapping adapters
    `map_key_to_msg`, `map_key_in_search_mode`, `map_key_in_normal_mode` (L381–413),
    plus the const `TTY_ERROR_KEY` (L22). Depends on crossterm, ratatui `Terminal`,
    tokio, `crate::client`, `crate::commands`, `crate::store`, `crate::cli`, `i18n::t`.
- `src/main.rs` L11 `mod tui;` and L362 `tui::browse(...)` MUST keep resolving
  unchanged: `mod tui;` resolves to `src/tui/mod.rs` once `src/tui.rs` is deleted, and
  `crate::tui::browse` stays reachable via a re-export in `mod.rs`.
- `tests/unit/tui.rs` is included via `#[cfg(test)] #[path = "../tests/unit/tui.rs"] mod tests;`
  and opens with `use super::*;`. It references **only** `browse`, `fetch_and_run`,
  `run_search`, `update`, `Model`, `Screen`, `Msg`, `Cmd`, `view` from the module — so
  `super` must surface exactly those. The test file itself stays **byte-identical**.

## Approach (decided)

- Create `src/tui/mod.rs` as the module root:
  - `mod model;` `mod view;` `mod shell;`
  - Re-export the names the test module's `use super::*;` and external callers need:
    `pub use model::{Cmd, Model, Msg, Screen, update};`, `pub use view::{view, view_detail};`,
    `pub(crate) use shell::{browse, fetch_and_run, run_search};`. (Preserve current
    visibility: `browse`/`view`/`view_detail`/`update`/the types are `pub`;
    `fetch_and_run`/`run_search` are `pub(crate)`.)
  - Move the `#[cfg(test)] #[path = "../tests/unit/tui.rs"] mod tests;` declaration here.
- Move each concern band verbatim into `model.rs` / `view.rs` / `shell.rs`. Add the
  minimal `use super::model::{...}` (and `use super::view::view;` in shell) so each
  submodule resolves the names it references. **No function body changes** — only the
  `use` headers move/adjust and `pub(super)`/`pub(crate)` visibility is set so the
  submodules can see each other's items (e.g. `update`, the `Model`/`Cmd` types, and
  `view` must be at least `pub(crate)`/`pub(super)` for shell+view to use them).
- Delete `src/tui.rs`.
- Update `docs/architecture.md`: the `tui` node in the **Module structure** Mermaid
  diagram (L62) gains its internal split — show `model` (pure core) / `view` / `shell`
  inside the `tui` boundary — and the surrounding prose notes the submodule layout.
  Same change (maintenance invariant).

## Vertical Demo

- **Given** the refactor has landed,
  **When** I run `docker compose run --rm dev cargo test`,
  **Then** the full suite (incl. all `tests/unit/tui.rs` cases, unchanged) passes, and
  `docker compose run --rm dev cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check` are clean.
- **And** `jira browse` behaves byte-for-byte as before (no observable change — this is
  a pure internal move).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | The full test suite passes through the new module path; `tests/unit/tui.rs` is byte-identical and every case still runs and passes (`use super::*` resolves via mod.rs re-exports) | command (`cargo test`) |
| AC2 | constraint | `crate::tui::browse` still resolves from `main.rs` (L362) and `mod tui;` (L11) is unchanged; the public API (`browse`, `view`, `view_detail`, `update`, `Model`/`Screen`/`Msg`/`Cmd`, `fetch_and_run`, `run_search`) keeps its prior visibility; no behavior change | inspection |
| AC3 | constraint | `model.rs` is the pure functional core: it imports **no** crossterm, ratatui, tokio, or `std::io` — only `crate::models` and `crate::render` (ADR 0007 §2); all terminal/network/clock I/O lives in `shell.rs` | inspection |
| AC4 | constraint | clippy `--all-targets` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean (no banners / commented-out code / superfluous comments; only the existing doc comments + non-obvious why-comments carry over) | command (clippy + fmt + comment_policy) |
| AC5 | constraint | No function body is rewritten — the split is a move; cyclomatic ≤10 / cognitive within the gate ceiling holds trivially (no logic added) | command (complexity) |
| AC6 | constraint | `docs/architecture.md` Module-structure diagram + prose reflect the `tui` submodule split (model/view/shell) in the same change (maintenance invariant) | inspection |

## Out of scope

- Any behavior, logic, or rendering change — this is a pure module move.
- Splitting `shell.rs` further (e.g. a separate `cmd.rs` for dispatch) — defer until it
  earns its own split.
- Moving `tests/unit/tui.rs` itself or splitting the test file per submodule — the test
  module stays one file, included from `mod.rs`, byte-identical.

## blocked_by

(none — isolated to the `tui` module; ADR 0007 §6 already decided the layout)
