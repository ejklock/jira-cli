---
type: Issue
title: Parallel unit tests fail intermittently on shared process globals
description: cargo test in parallel fails about 2 runs in 5 on trunk, a different test each time (language, cwd, TTL boundary); the serial run is green.
status: closed
timestamp: 2026-11-14T16:51:10Z
---

## 0068. Parallel unit tests fail intermittently on shared process globals

### Problem

The parallel unit suite fails intermittently on an unmodified trunk. Proof (2026-11-14): five runs of `cargo test --bin jira` in a worktree of `main` at c5edffd gave 3 green and 2 red, each red a different test: `store::cache::tests::task_list_cache_row_exactly_at_max_age_reads_some` and `commands::tests::current_core_not_in_git_repo_exits_2_with_zero_requests`. On the [issue 0063](/issues/0063-jira-mcp-serve-the-cli-as-mcp-tools-over-stdio.md) branch, parallel runs also failed `commands::tests::mine_core_builds_exact_jql_renders_table_and_exits_0` (pt-BR table headers: it reads the process-global language without `LANG_MUTEX`) and `tui::render_tests::selection_text_is_identical_regardless_of_the_current_scroll_offset`; `cargo test -- --test-threads=1` is green.

Likely causes (inference, not proved): tests that read process globals (language, current directory, wall clock at a TTL boundary) without the crate-wide lock.

### Scope

Find each shared global the failing tests read, serialize or inject it, and make the parallel suite green in repeated runs. No production behavior change.

### Acceptance

- AC1: `docker compose run --rm dev cargo test` (parallel) is green in 10 consecutive runs. (`verify_by: command`)
- AC2: no test is removed or weakened; the serial run keeps the same count. (`verify_by: command`)

### Decision

- Two causes (session, 2026-11-14): tests that assert English text read the process-global `LANGUAGE` (`src/i18n.rs`) without `LANG_MUTEX` while the i18n tests switch it to `pt_BR`, e.g. `tests/unit/commands.rs` `current_core_not_in_git_repo_exits_2_with_zero_requests` expects "not in a git repository"; and `tests/unit/store/cache.rs` `task_list_cache_row_exactly_at_max_age_reads_some` writes at `now` while `src/store/cache.rs` `read` takes a second `now_epoch_secs()`, so a second boundary between them fails it even in a serial run.
- Fix both (owner, 2026-11-14): an injectable clock for the task-list cache read, and every test whose assertion depends on the display language holds `LANG_MUTEX` with the language set. Not taken: the clock only, keeping `--test-threads=1` as the documented command; a thread-local language under `cfg(test)`.
- The lock is taken through one guard, `lock_language(lang)` in `tests/unit/support.rs`, which sets the language and restores "en" on drop; no test locks `LANG_MUTEX` directly (session, 2026-11-14). It lives in the test-support module, not in `src/i18n.rs`, so the production file stays unchanged.
- The language-dependent tests were enumerated by evidence: in a temporary copy where every thread not holding the lock saw `pt_BR`, 19 unit tests failed before the change and none after; `selection_text_is_identical_regardless_of_the_current_scroll_offset` was found separately (two renders with a language switch between them).

### Plan

- S1: the task-list cache read takes an injectable `now`, and the boundary test uses it | files: src/store/cache.rs, tests/unit/store/cache.rs | after: none
- S2: every language-dependent test holds `LANG_MUTEX` with its language set, through one shared helper; the parallel suite is green in 10 consecutive runs | files: tests/unit/** (except store/cache.rs), src/i18n.rs (test helper only, if it must live there) | after: S1

### Outcome

Delivered in commit 83cd0f7: the task-list cache decides freshness through `read_at(now)`, and every language-dependent test takes `lock_language` from `tests/unit/support.rs`. Proof: ten consecutive parallel `cargo test` runs green, and six more by review; the serial run passes 1044 tests; a probe that shows `pt_BR` to any test not holding the lock fails 19 unit tests on main and none after. Left as is: about 100 `set_language("en")` calls at test ends that the guard already makes redundant (review nit).
