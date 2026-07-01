---
type: Issue
title: "TUI test hygiene + view_list complexity refactor (debt)"
description: Two accumulated debts, no behavior change. (1) Refactor view_list (cognitive 15, over the 12 ceiling) by extracting its search-bar / error-banner / footer-hint branches into helpers so the QG stops flagging it on every view.rs-touching slice. (2) Lock crate::i18n::LANG_MUTEX in every language-dependent TUI render test so the process-global LANGUAGE race (lesson 3356) stops causing intermittent parallel-test failures and `cargo test` passes with default parallel threads.
status: done
tracker:
tags: [tui, browse, refactor, test-hygiene, debt, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# 0023 — TUI test hygiene + view_list complexity refactor (debt)

## Objective link

Maintenance under [ADR 0007](/adr/0007-browse-tui-elm-architecture.md) (browse TUI) — no
behavior or structure change, so no new ADR. Clears two recurring debts recorded in
lesson 3356 (LANGUAGE-global test race) and flagged by every recent `view.rs` slice
(view_list cognitive=15). Unblocks A4 (which touches `view.rs` again).

## Context manifest

- **Read first:** `src/tui/view.rs` — `view_list` (accrued cognitive 15 over P3's
  token-hint branch + issue 0020's `t()`-wrapping branches): it builds the layout
  constraints, conditionally renders the search bar and error banner, renders the table
  (empty vs non-empty), and builds the footer hint (base hint + `n mais` when a token is
  pending). `view_detail` and the A1/A2 rich-rendering helpers are NOT in scope.
- `src/i18n.rs` — `#[cfg(test)] pub(crate) LANG_MUTEX` (the crate-wide test lock) and
  `set_language`/`t`. The LANGUAGE static is a process global; tests that assert
  language-dependent rendered chrome must serialize on `LANG_MUTEX`.
- `tests/unit/tui.rs` — the TUI render tests. Some already lock `LANG_MUTEX` (the pt_BR
  ones from issue 0020 / A1 / A2); the **en-default** render tests that assert chrome
  (e.g. `view_empty_model_renders_no_issues_notice`,
  `view_list_shows_load_more_hint_when_token_pending`, the footer/header assertions) do
  NOT lock, so a concurrent pt_BR test can flip `LANGUAGE` under them → the intermittent
  parallel-run failures that currently force `-- --test-threads=1`.

## Approach (decided)

**Part 1 — view_list complexity (behavior-preserving refactor):**

- Extract cohesive helpers from `view_list`, e.g.: `list_footer_hint(model) -> String`
  (the base-hint + `n mais` logic), and private render helpers for the search bar and the
  error banner (each taking `&mut Frame` + the target chunk), and/or a table-builder helper
  for the empty-vs-non-empty rows. Aim: `view_list` cognitive ≤ 12 (prefer well under),
  cyclomatic ≤ 10, via guard clauses / extracted functions — not by merging behavior.
- The rendered output must be **byte-identical**: all existing `view_list` `TestBackend`
  assertions (en + pt_BR) stay green unchanged.

**Part 2 — LANG_MUTEX test hygiene:**

- Every test in `tests/unit/tui.rs` (and any other unit test) that renders
  language-dependent output and asserts it must lock `crate::i18n::LANG_MUTEX` for the
  duration and leave the language at `en` afterward (mirror the existing pt_BR test
  pattern: lock, `set_language(...)`, render, assert, `set_language("en")`). The
  en-default render tests must ALSO take the lock (they are the race victims), even though
  they do not call `set_language`.
- After the change, the FULL suite must pass under **default parallel** `cargo test` (no
  `--test-threads=1`) — that is the instrument proving the race is gone.

## Vertical Demo

- **Given** the repo,
- **When** I run `docker compose run --rm dev cargo test` (default parallel threads, no
  `--test-threads=1`) several times,
- **Then** it passes every time (no intermittent LANGUAGE-race failure), and
  `docker compose run --rm dev` arborist/QG no longer reports `view_list` over the cognitive
  ceiling.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | `view_list` cognitive complexity ≤ 12 and cyclomatic ≤ 10 after extracting helpers; the extracted helpers are each within the ceiling | command (complexity / arborist) |
| AC2 | behavior | `view_list` rendered output is byte-identical to before — all existing `view_list` `TestBackend` tests (en + pt_BR) pass unchanged | test (TestBackend) |
| AC3 | constraint | Every language-dependent TUI render test locks `crate::i18n::LANG_MUTEX` and restores `set_language("en")`; the full suite passes under DEFAULT parallel `cargo test` (no `--test-threads=1`) across repeated runs | command (`cargo test`, parallel) |
| AC4 | constraint | No production behavior change beyond the `view_list` refactor: `update`/`Model`/`Msg`/`Cmd`, `view_detail`, the A1/A2 rich helpers, and all catalogs are untouched | inspection |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; no superfluous comments/banners/commented-out code (the extracted helpers are named, not comment-explained) | command (clippy + fmt + comment_policy) |

## Out of scope

- Dependency-injecting the language to remove the process-global entirely (a larger
  refactor; the LANG_MUTEX discipline is the agreed pragmatic fix).
- `view_detail` / the A1/A2 rich-rendering paths; `map_key_in_normal_mode` (its complexity
  is a separate concern and it is not over the ceiling by the same measure).
- Any feature/behavior change.

## blocked_by

(none — pure debt cleanup; unblocks A4)
