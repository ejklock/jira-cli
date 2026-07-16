---
type: Issue
title: "H2 — centralize the JSON payload builders (client + commands) into support.rs"
description: Move build_issue_payload (95% duplicated between tests/unit/client.rs and tests/unit/commands.rs), build_myself_payload, and the Instance builder into tests/unit/support.rs, parametrized to cover both call sites; migrate client.rs + commands.rs. No behavior change, no assertion change. Second of three test-support consolidation slices (observation 55).
status: done
tracker:
tags: [test-hygiene, fixtures, duplication, debt, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# H2 — centralize the JSON payload builders (client + commands)

> **Delivered 2026-07-16** via the re-sliced test-support debt program
> (plan `589`, slice B). `build_issue_payload` diverged in 9 fields between
> `client.rs` and `commands.rs`, so it was consolidated as a shared
> parametrized builder (`IssuePayloadOptions` + thin per-file wrappers) in
> `support.rs`; `build_myself_payload` (an exact duplicate) was moved
> verbatim. The two exhaustive `Issue{..}` literals in those files were also
> spread onto `..issue(key)`.

## Objective link

Maintenance under [ADR 0007](/adr/0007-browse-tui-elm-architecture.md) — test hygiene, no ADR.
Second third of observation 55. Persisted plan `510`. Depends on H1 (issue 0027) for `support.rs`.

## Context manifest

- **Duplicated (95% identical):** `build_issue_payload()` in `tests/unit/client.rs:16-99` AND
  `tests/unit/commands.rs:442-521` — both build the full Jira Cloud `/rest/api/3/issue` GET
  response (fields.summary/status/issuetype/assignee/priority/created/updated/description ADF/
  comment.comments[]). Minor variations: reporter present in commands, description text differs.
- Also: `build_myself_payload()` (`client.rs:101`), `make_instance(base_url)` (`client.rs:6`),
  `make_test_instance()` (`tui.rs:40`), `build_search_issue`/`build_search_payload`
  (`commands.rs:1310-1357`) — payload builders that can share a home.
- `tests/unit/support.rs` exists from H1.

## Approach (decided — see plan 510)

- Move `build_issue_payload` into `support.rs`, **parametrized** (e.g. optional reporter /
  description override) so both `client.rs` and `commands.rs` call it with their specific needs and
  every current assertion still holds. Move `build_myself_payload` and a single `instance(...)`
  builder too.
- Migrate `client.rs` + `commands.rs` to `use crate::test_support::*` and delete the local
  duplicates. Keep any genuinely call-site-specific payload shaping local.
- **No assertion may change.**

## Vertical Demo

- **Given** the repo, **when** I run `docker compose run --rm dev cargo test`, **then** client.rs +
  commands.rs pass sourcing their payloads from `crate::test_support`, and the jscpd
  `build_issue_payload` clone is gone.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | `build_issue_payload` (parametrized), `build_myself_payload`, and an `instance(...)` builder live in `support.rs`; `client.rs` + `commands.rs` use them with NO local duplicate | inspection |
| AC2 | behavior | Every client.rs + commands.rs test passes UNCHANGED (no assertion edits) | test (full suite) |
| AC3 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; the client↔commands payload duplication is eliminated (jscpd) | command |
| AC4 | constraint | No production behavior change; no superfluous comments/banners/commented-out code | inspection |

## Out of scope

- The ADF/Issue builders (H1); `cache.rs`/`agent_json.rs`/`models.rs` Issue fixtures (H3). Any src change.

## blocked_by

- [0027](/issues/0027-h1-test-support-module-and-adf-issue-builders.md) (H1 — provides `support.rs`)
