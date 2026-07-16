---
type: Issue
title: "H1 — shared tests/unit/support.rs (ADF + Issue builders), migrate render + tui tests"
description: Create a shared tests/unit/support.rs test-support module (ADF fixture builders, duedate_offset_from_today, assignee()/comment() helpers, and an issue(key) builder), wire it into src/main.rs via #[cfg(test)] mod test_support, and migrate tests/unit/render.rs + tests/unit/tui.rs to use it — deleting their duplicated local fixtures. No production behavior change, no test assertion change. First of three test-support consolidation slices (observation 55).
status: done
tracker:
tags: [test-hygiene, fixtures, duplication, debt, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# H1 — shared `tests/unit/support.rs` + migrate render/tui tests

> **Delivered 2026-07-16** via the re-sliced test-support debt program
> (plan `589`, slice A). This manifest had drifted: `support.rs` already
> existed and was wired, and issue [0015](/issues/0015-split-tui-into-submodule.md)
> had split `tui.rs` into a `tui/` submodule after H1 was written. Slice A
> migrated the TUI fixtures and de-duplicated `make_test_instance` +
> `build_search_payload_with_key` (the new duplicates the split introduced,
> unlisted here) into `support.rs`.

## Objective link

Maintenance under [ADR 0007](/adr/0007-browse-tui-elm-architecture.md) — pure test hygiene, no
new ADR. Resolves the first third of observation 55 (duplicated per-module test fixtures trip the
jscpd gate on every `Issue`-touching slice). Persisted plan `510` (`test-support-issue-builder`).

## Context manifest

- **Harness fact:** each `src/*.rs` includes its unit tests via
  `#[cfg(test)] #[path = "../tests/unit/FILE.rs"] mod tests;` at the module boundary. There is
  **no `tests/unit/mod.rs` and no shared test module today** — every test file redefines its
  fixtures.
- **Duplicated ADF builders** (identical): `doc`/`paragraph`/`text`/`marked_text`/`link_mark`/`mark`
  in `tests/unit/render.rs:11-77` AND `tests/unit/tui.rs:16-38`. render.rs also has unique nodes
  (`heading`, `code_block`, `blockquote`, `panel`, `rule_block`, `bullet_list`, `ordered_list`,
  `list_item`, `hard_break`, `custom_node`, `plain_paragraph`, `marked_paragraph`).
- **Duplicated helper:** `duedate_offset_from_today(days)` — `render.rs:829` AND `tui.rs:2008`
  (identical).
- **Issue fixtures to migrate here:** `tests/unit/tui.rs:81` `make_issue` (+ variants
  `make_issue_with_styled_description:154`, `make_issue_with_comments:179`,
  `make_issue_with_two_links:189`, `make_issue_with_duedate:2018`) and `tests/unit/render.rs:83`
  `sample_issue`. Both assert specific rendered values — preserve them via `..issue(key)` spread.
- **Domain types:** `Issue`, `IssueAssignee`, `IssueComment` in `src/models.rs`.

## Approach (decided — see plan 510)

- Create `tests/unit/support.rs` with `pub(crate)` items: the ADF builders (all of them — the
  shared six plus render.rs's unique nodes, so support.rs is the single home for ADF fixtures),
  `duedate_offset_from_today`, `assignee(display_name, account_id)`, `comment(...)`, and
  `issue(key: &str) -> Issue` with neutral defaults covering every `Issue` field.
- Wire it in `src/main.rs`: `#[cfg(test)] #[path = "../tests/unit/support.rs"] mod test_support;`.
- Migrate `tests/unit/render.rs` and `tests/unit/tui.rs`: `use crate::test_support::*;`, delete the
  local ADF builders / `duedate_offset_from_today` / `sample_issue` / `make_issue*`, and rebuild each
  fixture as `Issue { <the fields this test asserts>, ..issue(key) }`. Variants
  (`make_issue_with_*`) become thin wrappers over `issue()` + spread, kept local only if they are
  render/tui-specific composition.
- **No assertion may change.** Every render.rs + tui.rs test passes byte-identically; LANG_MUTEX
  stays locked in every language-dependent render test (issue 0023 discipline).

## Vertical Demo

- **Given** the repo, **when** I run `docker compose run --rm dev cargo test` (default parallel),
  **then** it passes with render.rs + tui.rs sourcing their ADF/Issue fixtures from
  `crate::test_support`, and **when** the QG runs jscpd, **then** the render↔tui ADF/`duedate_offset`
  clones are gone.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | `tests/unit/support.rs` exists with `pub(crate)` ADF builders + `duedate_offset_from_today` + `assignee`/`comment` + `issue(key)`; wired via `#[cfg(test)] #[path] mod test_support` in `src/main.rs` | inspection + build |
| AC2 | constraint | `render.rs` + `tui.rs` `use crate::test_support::*` and contain NO local copy of the shared ADF builders / `duedate_offset_from_today` / `sample_issue`/`make_issue` exhaustive literal; render-unique ADF nodes live in support.rs | inspection |
| AC3 | behavior | Every render.rs + tui.rs test passes UNCHANGED (no assertion edits); LANG_MUTEX still locked in language-dependent render tests | test (full suite) |
| AC4 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; the render↔tui ADF/`duedate_offset` duplication is eliminated (jscpd) | command |
| AC5 | constraint | No production behavior change (only `src/main.rs` gains the `#[cfg(test)]` test_support wiring); no superfluous comments/banners/commented-out code | inspection |

## Out of scope

- `client.rs`/`commands.rs` payload builders (H2, issue 0028); `cache.rs`/`agent_json.rs`/`models.rs`
  Issue fixtures (H3, issue 0029). Any production/src change beyond the main.rs wiring.

## blocked_by

(none — first hygiene slice; establishes support.rs for H2/H3)
