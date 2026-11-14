---
type: Issue
title: Parallel unit tests fail intermittently on shared process globals
description: cargo test in parallel fails about 2 runs in 5 on trunk, a different test each time (language, cwd, TTL boundary); the serial run is green.
status: open
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
