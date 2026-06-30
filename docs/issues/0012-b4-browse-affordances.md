---
type: Issue
title: "B4 — browse TUI: read affordances (open link, copy key)"
description: Pressing 'o' opens {base_url}/browse/{KEY} in the system browser; 'y' copies the selected KEY to the clipboard. Both read-only; the URL build + key selection are pure-tested, the spawn/clipboard call is the untested Humble Object shell. A shared issue_browse_url helper is extracted so the URL format has one home.
status: done
tracker:
tags: [tui, browse, phase2, affordances]
timestamp: 2026-06-30T00:00:00Z
---

# B4 — browse TUI: read affordances (open link, copy key)

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R4 → [BDR 0006](/bdr/0006-browse-tui-interactions.md)
S4 (open/copy) → architecture [ADR 0007](/adr/0007-browse-tui-elm-architecture.md). Stacks on the
B1 list ([issue 0009](/issues/0009-b1-browse-list.md)). Final slice of the browse-TUI Phase 2.

## Context manifest

- **Read first:** `src/tui.rs` (B3 state — `Model { rows, selected, screen, detail, detail_scroll,
  search, error }`, `Msg`/`Cmd`, pure `update`/`update_*`, `map_key_in_search_mode`/
  `map_key_in_normal_mode`, `view_list`/`view_detail`, `run_tui`/`draw_loop`/`dispatch_cmd` with the
  `block_in_place`+`block_on` glue, async `fetch_and_run`/`load_detail`/`run_search`),
  `src/render.rs` (`render_issue_human` L167 — the `format!("{}/browse/{}",
  base_url.trim_end_matches('/'), issue.key)` at L174), `src/agent_json.rs` (the SAME URL format at
  L17), `src/main.rs` (`dispatch_browse` L354 — the `instance` carries `base_url`; `current_git_branch`
  L364 is the existing `std::process::Command` shell-out precedent).
- **Extract the URL helper (one home, no drift):** add `pub fn issue_browse_url(base_url: &str, key:
  &str) -> String` to `src/render.rs` returning `format!("{}/browse/{}",
  base_url.trim_end_matches('/'), key)`. Refactor `render_issue_human` (render.rs:174) AND
  `agent_json` issue_object (agent_json.rs:17) to call it — this consolidates the existing two copies
  and realizes the stated "agent_json derives from the same render helpers" invariant. The TUI uses
  the same helper, so the URL format never drifts across the three sites.
- **TEA growth (still one file `src/tui.rs`):** carry the instance base URL in the Model — add
  `base_url: String` (seeded in `run_tui` from `instance.base_url`); update every Model constructor +
  the `make_*_model` test helpers. Grow `Msg` with `OpenLink`, `CopyKey`. Grow `Cmd` with
  `OpenUrl(String)` (the full browse URL) and `CopyToClipboard(String)` (the KEY). Keep `update`
  pure: `OpenLink` → if the list is non-empty, emit `vec![Cmd::OpenUrl(render::issue_browse_url(
  &model.base_url, &model.rows[model.selected].key))]`; `CopyKey` → if non-empty, emit
  `vec![Cmd::CopyToClipboard(model.rows[model.selected].key.clone())]`; empty list → no-op (no Cmd).
  No async/I/O in `update`.
- **Key mapping (`map_key_in_normal_mode`):** when `search` is inactive, `Char('o')` → `OpenLink`,
  `Char('y')` → `CopyKey`; the B1-B3 mappings stay. (In search mode these are typed into the query —
  unchanged.)
- **Execute the affordance Cmds in the shell (Humble Object, untested):** in `dispatch_cmd`, handle
  `Cmd::OpenUrl(url)` by spawning the platform opener via `std::process::Command` (macOS `open`,
  else `xdg-open`) and `Cmd::CopyToClipboard(key)` by piping the key to the platform clipboard tool
  (macOS `pbcopy`, else `xclip -selection clipboard`, falling back to `wl-copy`). Best-effort: ignore
  a spawn failure (no display / no clipboard tool / inside a container) — never crash, never leave the
  terminal broken, no panic. NO new crate dependency — mirror `current_git_branch`'s
  `std::process::Command` shell-out. These spawn calls are the untested shell; the URL build + key
  selection are pure-tested.
- **Read-only invariant:** `OpenUrl`/`CopyToClipboard` are read affordances — they do NOT call any
  Jira write API. No write `Cmd` is introduced (constitution boundary).
- **Footer hint:** optionally extend the existing footer hint to mention `o open`/`y copy` only if it
  reuses existing `t()` keys; do NOT add catalog keys.

## Vertical Demo

- **Given** a TTY and the list with a selected issue,
  **When** I press `o`,
  **Then** `{base_url}/browse/{KEY}` opens in the system browser; **When** I press `y`, **Then** the
  selected KEY is on the clipboard; `q` quits.
- **Edge:** **Given** no browser/clipboard tool is available (e.g. headless / container),
  **When** I press `o` or `y`, **Then** the TUI stays usable — the action is a best-effort no-op, no
  crash, no broken terminal.
- **Edge:** **Given** an empty list, **When** I press `o`/`y`, **Then** nothing happens (no Cmd).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `update(OpenLink)` on a non-empty list emits `Cmd::OpenUrl` with the `{base_url trimmed}/browse/{selected KEY}` URL; empty list → no-op (no Cmd) | test |
| AC2 | behavior | `update(CopyKey)` on a non-empty list emits `Cmd::CopyToClipboard` with the selected KEY; empty list → no-op (no Cmd) | test |
| AC3 | behavior | `issue_browse_url(base_url, key)` builds `{base_url trimmed}/browse/{key}` (trailing slash trimmed); `render_issue_human` and `agent_json` produce the identical URL through the shared helper | test |
| AC4 | constraint | No superfluous comments / banners / commented-out code; cyclomatic ≤10 (≤8 new fns) / cognitive within ceiling | command (comment_policy + complexity) |
| AC5 | constraint | NO write Cmd / no Jira write API introduced — the affordances are read-only; no new crate dependency (shell-out via std::process::Command) | inspection (Reviewer) + build |
| AC6 | constraint | Honors ADR 0007: `update` + `issue_browse_url` pure and tested off-terminal; only the `dispatch_cmd` spawn glue is the untested Humble Object shell; `src/tui.rs` stays one file; the URL helper has one home (render.rs) reused by all three sites | inspection (Reviewer) |

## Out of scope

- Saved filters / search history / multi-page paging.
- A configurable opener/clipboard command (use the platform defaults best-effort).
- Splitting `src/tui.rs` into a `src/tui/` submodule — a later dedicated refactor slice (ADR 0007).

## blocked_by

[0009](/issues/0009-b1-browse-list.md)
