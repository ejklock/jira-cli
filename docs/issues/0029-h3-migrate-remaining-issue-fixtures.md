---
type: Issue
title: "H3 — migrate the remaining Issue fixtures (cache/agent_json/models) to the shared builder"
description: Migrate the last exhaustive Issue{..} fixtures — tests/unit/store/cache.rs make_issue, tests/unit/agent_json.rs sample_issue, and tests/unit/models.rs inline Issue literal — to the shared crate::test_support::issue()+spread builder, so every Issue{..} literal lives once in support.rs and a future Issue field only touches it. No behavior change, no assertion change. Third of three test-support consolidation slices (observation 55).
status: done
tracker:
tags: [test-hygiene, fixtures, duplication, debt, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# H3 — migrate the remaining Issue fixtures (cache / agent_json / models)

> **Delivered 2026-07-16** via the re-sliced test-support debt program
> (plan `589`, slice C). `cache.rs make_issue`, `agent_json.rs sample_issue`
> and the `models.rs` serde-roundtrip literal were all migrated to
> `..crate::test_support::issue(key)` spreads. After this slice the only
> exhaustive all-field `Issue{..}` literal in `tests/unit/**` is `support.rs`'s
> `issue()` builder — adding a future `Issue` field now touches one file.

## Objective link

Maintenance under [ADR 0007](/adr/0007-browse-tui-elm-architecture.md) — test hygiene, no ADR.
Final third of observation 55 — after this, adding a field to `Issue` touches only `support.rs`.
Persisted plan `510`. Depends on H1 (issue 0027) for the `issue()` builder.

## Context manifest

- **Issue fixtures still local after H1:** `tests/unit/store/cache.rs:375` `make_issue(key)`,
  `tests/unit/agent_json.rs:4` `sample_issue()`, and the inline `Issue { .. }` literal in
  `tests/unit/models.rs:6` (`issue_roundtrips_through_serde`, which sets `duedate: Some(..)`).
- `agent_json.rs sample_issue` and `cache.rs make_issue` assert specific rendered/cached values —
  preserve them via `..issue(key)` spread.
- `models.rs`'s serde-roundtrip test asserts a full round-trip; if it must pin every field
  explicitly, it may keep an explicit literal — note it if so (it is the one legitimate exception).
- `tests/unit/support.rs` + `issue(key)` builder exist from H1.

## Approach (decided — see plan 510)

- Migrate `cache.rs make_issue`, `agent_json.rs sample_issue`, and the `models.rs` inline literal
  to `Issue { <asserted fields>, ..crate::test_support::issue(key) }`.
- After migration, the only exhaustive `Issue { .. }` literal in the codebase is `issue()` in
  `support.rs` (plus, if unavoidable, the single `models.rs` serde-roundtrip literal — documented).
- **No assertion may change.**

## Vertical Demo

- **Given** the repo, **when** I run `docker compose run --rm dev cargo test`, **then**
  cache/agent_json/models tests pass sourcing their Issue from `crate::test_support::issue()`, and
  **when** a new `Issue` field is later added, **then** only `support.rs` needs updating (the
  obs-55 pain is gone).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | `cache.rs`/`agent_json.rs`/`models.rs` Issue fixtures rebuilt via `crate::test_support::issue()` + spread; no local exhaustive `Issue { .. }` literal remains except `support.rs`'s builder (and, if unavoidable, the documented `models.rs` serde-roundtrip literal) | inspection |
| AC2 | behavior | Every cache/agent_json/models test passes UNCHANGED (no assertion edits) | test (full suite) |
| AC3 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; adding a future `Issue` field now touches only `support.rs` (verify by inspection of the remaining literals) | command + inspection |
| AC4 | constraint | No production behavior change; no superfluous comments/banners/commented-out code | inspection |

## Out of scope

- The ADF/Issue builders + render/tui (H1); the JSON payload builders (H2). Any src change.

## blocked_by

- [0027](/issues/0027-h1-test-support-module-and-adf-issue-builders.md) (H1 — provides the `issue()` builder)
