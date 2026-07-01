---
type: Issue
title: "A3a — due date on the model + relative formatter + CLI get Due line"
description: Add duedate:Option<String> to Issue and map it in the client; add a pure relative_due formatter (today/tomorrow/in N days/overdue by N days) built on stdlib days_from_civil (no chrono) with English-source i18n keys + new pt_BR entries; render a Due line in the CLI get. First of two A3 slices (ADR 0013), Group A parity.
status: done
tracker:
tags: [duedate, i18n, render, cli, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# A3a — due date on the model + relative formatter + CLI `get` Due line

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) (`get`), realizing
[ADR 0013](/adr/0013-relative-due-date-rendering.md) under the i18n contract
[ADR 0006](/adr/0006-i18n-interpolation-contract.md). First A3 slice; A3b (issue 0026) reuses
this formatter for the TUI + agent_json. Persisted plan `509` (`a3-relative-duedate`).

## Context manifest

- **Read first:** `src/models.rs` — `Issue` struct (L25–40): fields key/summary/status/
  status_category/issue_type/assignee/reporter/priority/created/updated/description/comments;
  derives Debug/Clone/PartialEq/Eq/Serialize/Deserialize. **No `duedate` today** — add it.
- `src/client.rs` — `map_gouqi_issue` (L129–170) builds a domain `Issue` from gouqi via typed
  accessors, e.g. `raw.created().map(|dt| dt.to_string())`, `raw.priority()`. Add `duedate`
  following the same pattern: prefer a gouqi due-date accessor if one exists; otherwise read the
  raw `duedate` field as `Option<String>` (Jira returns `"YYYY-MM-DD"` or null).
- `src/render.rs` — `render_issue_human` (L423–483) emits `Created` (L468–470) / `Updated`
  (L471–473) lines via `t()`. Slot the `Due` line **after `Updated`**. This file also hosts the
  new pure formatter.
- `src/i18n.rs` — `t(s: &str) -> String` (L81–90, identity under en, catalog under pt_BR);
  `tf(template: &str, args: &[(&str, &str)]) -> String` (L106–118, translate template then
  substitute `{name}` tokens). **No `locales/en.json` exists — en is identity** (so use English
  source strings as keys, never the orphan symbolic `due_*` keys).
- `locales/pt_BR.json` — already has `"Due": "Prazo"`. The symbolic `due_*` keys (L122–128) are
  orphans — **do not use them**. Add the new English-source keys (below).
- `Cargo.toml` — **no chrono/time crate**; stdlib only (`SystemTime`, manual date arithmetic in
  `src/store/mod.rs` `secs_to_utc_parts`/`now_iso`). Do NOT add a date dependency.

## Approach (decided — see ADR 0013)

- **Model:** add `pub duedate: Option<String>` to `Issue` (raw `"YYYY-MM-DD"` or `None`).
- **Client:** `map_gouqi_issue` sets `duedate` from gouqi's due-date accessor if present, else the
  raw `duedate` field, as `Option<String>`.
- **Pure formatter in `src/render.rs`** (no I/O, `today` injected for testability):
  - `fn days_from_civil(y: i64, m: i64, d: i64) -> i64` — Howard Hinnant's civil-to-days algorithm.
  - `fn relative_due(duedate: &str, today_days: i64) -> Option<String>` — parse `"YYYY-MM-DD"`
    (`split('-')` → 3 integer parts; return `None` on any parse failure), `delta = due_days -
    today_days`, bucket:
    - `0` → `t("today")`; `1` → `t("tomorrow")`; `>= 2` → `tf("in {n} days", &[("n", &delta.to_string())])`;
      `-1` → `t("overdue by 1 day")`; `<= -2` → `tf("overdue by {n} days", &[("n", &(-delta).to_string())])`.
  - The `today_days` the CLI passes is derived from `SystemTime::now()` (reuse the existing
    stdlib date extraction; e.g. parse the date portion of `now_iso()` through `days_from_civil`).
- **CLI `get`:** in `render_issue_human`, after the `Updated` line, when `issue.duedate` is `Some`
  and `relative_due` returns `Some`, emit `writeln!(out, "  {}: {due}", t("Due"))`; omit otherwise.
- **New pt_BR keys** in `locales/pt_BR.json`: `"today": "hoje"`, `"tomorrow": "amanhã"`,
  `"in {n} days": "em {n} dias"`, `"overdue by 1 day": "atrasada há 1 dia"`,
  `"overdue by {n} days": "atrasada há {n} dias"`. (`"Due"` already maps to `"Prazo"`.)

## Vertical Demo

- **Given** an issue whose `duedate` is 3 days from today,
- **When** I run `jira get KEY`,
- **Then** the output includes a `Due: in 3 days` line (pt_BR: `Prazo: em 3 dias`) after `Updated`;
- **And** an issue due yesterday shows `Due: overdue by 1 day` (`Prazo: atrasada há 1 dia`), due
  today shows `Due: today` (`Prazo: hoje`), and an issue with no due date shows **no** `Due` line.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `Issue` gains `duedate: Option<String>`; `map_gouqi_issue` populates it (gouqi accessor or raw `duedate` field) as `Option<String>` | test (unit, client mapping) |
| AC2 | behavior | `relative_due` maps day-delta to the correct bucket — `today` (0), `tomorrow` (1), `in {n} days` (≥2), `overdue by 1 day` (−1), `overdue by {n} days` (≤−2), `None` for unparseable/none — verified by a table-driven unit test; `days_from_civil` is correct for known dates | test (unit, table) |
| AC3 | behavior | `jira get` renders `{t("Due")}: {relative}` after `Updated` when `duedate` is `Some` and parses, and omits the line otherwise; asserted in both en and pt_BR | test (unit/integration) |
| AC4 | constraint | i18n uses English-source keys (not the symbolic `due_*`); the 5 new pt_BR entries exist and en renders the English source verbatim (identity-under-en per ADR 0006) | test + inspection |
| AC5 | constraint | No `chrono`/date crate added (`Cargo.toml` unchanged for deps); `relative_due`/`days_from_civil` are pure (no I/O); cyclomatic ≤10 (≤8 new fns) / cognitive within ceiling; a mutant flipping a bucket boundary or the singular/plural overdue is killed by AC2 | command (complexity + mutation) + inspection |
| AC6 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; full suite green under DEFAULT parallel `cargo test`; no superfluous comments/banners/commented-out code | command |

## Out of scope

- The browse TUI detail Due line and the raw `duedate` in `agent_json` — that is A3b (issue 0026).
- Overdue coloring / `DueStyle`; absolute-date fallback for far-future dates; `IssueRow`/list-view
  due date; removing the orphan symbolic `due_*` catalog keys.

## blocked_by

(none — first A3 slice)
