---
type: Issue
title: "A3b — relative Due line in the browse TUI detail + raw duedate in agent_json"
description: Show the relative due date (reusing A3a's relative_due formatter) in the browse TUI detail after the Assignee line, text-only; and add the raw duedate (YYYY-MM-DD) to the agent_json issue_object additively. Second of two A3 slices (ADR 0013), Group A parity.
status: done
tracker:
tags: [duedate, tui, agent-json, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# A3b — relative Due line in the browse TUI detail + raw `duedate` in agent_json

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) (browse detail) +
[ADR 0004](/adr/0004-agent-json-output-contract.md) (agent_json), realizing
[ADR 0013](/adr/0013-relative-due-date-rendering.md). Second A3 slice; reuses A3a's
`relative_due`. Persisted plan `509` (`a3-relative-duedate`).

## Context manifest

- **Read first:** `src/tui/view.rs` — `view_detail` (L174–241) builds the `lines` vec
  (L213–222): key, summary, blank, `Status`, `Type`, `Assignee` (L219), blank, `Description`.
  Slot a `Due` line **after the Assignee line (L219)**, before the blank at L220. Reuse
  `crate::render::relative_due` (A3a) with a `today_days` computed the same way the CLI does.
- `src/render.rs` — `relative_due(duedate: &str, today_days: i64) -> Option<String>` and
  `days_from_civil` (added in A3a). Make `relative_due` reachable from the TUI (`pub`/`pub(crate)`
  as needed). Do NOT re-implement the formatter.
- `src/agent_json.rs` — `issue_object` (L10–73) builds the curated agent JSON (ADR 0004); current
  fields end with `updated`, `url`, `description`, `comments`. Add a **raw** `"duedate"` field
  (the `Issue.duedate` string as-is, or null/omitted when `None`) — NOT the localized relative
  string.
- `src/models.rs` — `Issue.duedate: Option<String>` (added in A3a).
- `locales/pt_BR.json` — the A3a keys (`today`/`tomorrow`/`in {n} days`/`overdue by …`) already
  exist; `"Due": "Prazo"` already exists. No new keys.
- `tests/unit/tui.rs` — every language-dependent render test locks `crate::i18n::LANG_MUTEX`
  (issue 0023 discipline).

## Approach (decided — see ADR 0013)

- **TUI:** in `view_detail`, after the `Assignee` line, when `issue.duedate` is `Some` and
  `relative_due(duedate, today_days)` returns `Some`, push
  `Line::from(format!("{}: {due}", t("Due")))`; omit otherwise. Text-only (no color). Compute
  `today_days` via the same stdlib path the CLI uses (reuse the helper A3a introduced rather than
  duplicating it).
- **agent_json:** in `issue_object`, add `"duedate": issue.duedate` (raw string / null) to the
  JSON object, keeping the field additive and machine-readable.

## Vertical Demo

- **Given** `jira browse` and an issue due in 3 days,
- **When** I open its detail,
- **Then** a `Due: in 3 days` line (pt_BR `Prazo: em 3 dias`) appears after the Assignee line;
- **And** an issue with no due date shows no `Due` line;
- **And** `jira get KEY --agent-json` includes a raw `"duedate": "YYYY-MM-DD"` field.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `view_detail` renders `{t("Due")}: {relative}` after the Assignee line when `duedate` is `Some` and parses, omitted otherwise; asserted via `TestBackend` in en and pt_BR (LANG_MUTEX locked) | test (TestBackend) |
| AC2 | behavior | `agent_json` `issue_object` carries a raw `"duedate"` (the `YYYY-MM-DD` string) additively, null/omitted when `None`; the localized relative string is NOT put in agent_json | test (unit, agent_json) |
| AC3 | constraint | The TUI reuses `crate::render::relative_due` (no re-implementation / no duplicated date math); no new i18n key; no new `Msg`/`Cmd`/`Model` field | inspection |
| AC4 | constraint | No behavior change beyond the Due line + agent_json field; `view_detail` stays within the complexity ceiling; every language-dependent render test locks `LANG_MUTEX` | inspection + command (complexity) |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; no superfluous comments/banners/commented-out code | command |

## Out of scope

- The `relative_due`/`days_from_civil` formatter and the CLI `get` line (A3a, issue 0025).
- Overdue coloring / `DueStyle`; absolute-date fallback; `IssueRow`/list-view due date.

## blocked_by

- [0025](/issues/0025-a3a-duedate-formatter-and-cli-get.md) (A3a — provides `relative_due` + `Issue.duedate`)
