---
type: ADR
title: "Relative due-date rendering (stdlib date math, English-source i18n keys)"
status: Accepted
supersedes:
superseded_by:
tags: [duedate, i18n, render, tui, agent-json, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# ADR 0013 — Relative due-date rendering (stdlib date math, English-source i18n keys)

## Status

Accepted. Realized by issues 0025 (A3a) and 0026 (A3b).

## Context

Group A parity slice A3: surface a Jira issue's **due date** as a human relative string
("today" / "tomorrow" / "in N days" / "overdue by N days") in the CLI `get` and the browse TUI
detail. The fork base active-collab-cli has this (its `relative_due` / `day_word` in
`src/tui/model.rs`), and the pt_BR catalog still carries its **orphan** symbolic keys
(`due_today`, `due_tomorrow`, `due_in`, `due_overdue`, `due_day`, `due_days`, `due_none`).

Three facts from the codebase shape the decision:

1. **`Issue` has no `duedate` field** (`src/models.rs`); `map_gouqi_issue` (`src/client.rs`)
   maps gouqi's typed accessors (`raw.created().map(|dt| dt.to_string())`, etc.). So this is a
   vertical slice down through model + client, not a presentation-only change (unlike A1/A2/A4).
2. **jira-cli's i18n is identity-under-en** ([ADR 0006](/adr/0006-i18n-interpolation-contract.md)):
   there is **no `locales/en.json`** — `t(s)` returns `s` verbatim under en and looks `s` up in
   the pt_BR catalog under pt_BR. So English source strings ARE the keys. The orphan **symbolic**
   `due_*` keys therefore CANNOT be reused: `t("due_today")` would render the literal
   `"due_today"` under en. They are AC-cli leftovers keyed for a catalog-with-symbolic-keys model
   jira-cli does not use.
3. **No `chrono`/`time` crate** — jira-cli is stdlib-only for time (`std::time::SystemTime` +
   a manual `secs_to_utc_parts`). Adding a date library for one field is disproportionate.

## Decision

1. **Model + client:** add `duedate: Option<String>` to `Issue` (raw Jira `"YYYY-MM-DD"` string,
   or `None`). `map_gouqi_issue` populates it from gouqi's due-date accessor if one exists, else
   from the raw `duedate` field, as an `Option<String>`.
2. **Pure relative formatter in the domain core** (`src/render.rs`, no I/O, `today` injected):
   `relative_due(duedate: &str, today_days: i64) -> Option<String>`. It parses `"YYYY-MM-DD"`
   (split on `-`), converts both the due date and today to a day count via a pure
   `days_from_civil(y, m, d) -> i64` (Howard Hinnant's algorithm — ~15 lines, matches the existing
   no-chrono manual-date-arithmetic posture), computes `delta = due_days - today_days`, and buckets:
   - `delta == 0` → `t("today")`
   - `delta == 1` → `t("tomorrow")`
   - `delta >= 2` → `tf("in {n} days", [("n", delta)])`
   - `delta == -1` → `t("overdue by 1 day")`
   - `delta <= -2` → `tf("overdue by {n} days", [("n", -delta)])`
   - unparseable input → `None` (caller omits the line).
   Singular occurs only for the exact `-1` case (a fixed string), avoiding nested unit
   substitution; future is always plural (`delta >= 2`). No absolute-date fallback for the far
   future (mirrors AC-cli, which always shows "in N days").
3. **i18n keys are English source strings** (per ADR 0006), NOT the orphan symbolic keys. New
   pt_BR entries: `"today"→"hoje"`, `"tomorrow"→"amanhã"`, `"in {n} days"→"em {n} dias"`,
   `"overdue by 1 day"→"atrasada há 1 dia"`, `"overdue by {n} days"→"atrasada há {n} dias"`. The
   `Due` field label already exists (`"Due"→"Prazo"`). The orphan `due_*` keys are left untouched
   (removing them is out of scope, consistent with the other AC-cli catalog leftovers).
4. **Display surfaces:**
   - CLI `get` (`render_issue_human`): a `{t("Due")}: {relative}` line after `Updated`, rendered
     only when `duedate` is `Some` and parses; omitted otherwise (mirrors the description/comments
     omission). **(A3a)**
   - Browse TUI detail (`view_detail`): a `{t("Due")}: {relative}` line after the Assignee line,
     reusing the same `relative_due` formatter, text-only. **(A3b)**
   - `agent_json` (`issue_object`, [ADR 0004](/adr/0004-agent-json-output-contract.md)): a **raw**
     `"duedate": "YYYY-MM-DD"` (or omitted/null), NOT the localized relative string — the agent
     contract carries machine-readable raw data. **(A3b)**
5. **No color/style** for overdue in this slice (AC-cli returns a `DueStyle`; jira-cli defers
   coloring). Text-only relative string.

## Slicing

- **A3a** (issue 0025): `duedate` on the model + client, the pure `relative_due`/`days_from_civil`
  formatter, the new pt_BR keys, and the CLI `get` Due line. Demoable: `jira get KEY`.
- **A3b** (issue 0026): the Due line in the browse TUI detail (reuses the A3a formatter) + raw
  `duedate` in `agent_json`. Demoable: `jira browse` detail + `--agent-json`. Depends on A3a.

## Consequences

- **Positive:** due dates become visible and localized across `get`, browse, and (raw) agent_json;
  the formatter is a pure, table-testable domain function; no new dependency; honors the ADR 0006
  identity-under-en contract.
- **Negative / deferred:** no overdue coloring; no absolute-date fallback for far-future dates;
  the orphan symbolic `due_*` keys remain in the catalog (cosmetic debt).
- **agent_json contract (ADR 0004)** gains one additive optional field (`duedate`) — backward
  compatible.

## Alternatives considered

- **Reuse the orphan symbolic `due_*` keys.** Rejected: they break under the identity-under-en
  contract (`t("due_today")` → `"due_today"` in English). English-source keys are the jira-cli way.
- **Add the `chrono` crate.** Rejected: a heavy dependency for parsing one `YYYY-MM-DD` and a day
  delta; the pure `days_from_civil` is ~15 lines and matches the existing stdlib-only date posture.
- **Store the relative string on the model / in agent_json.** Rejected: the model and agent_json
  carry raw data; localization/relativization is a render-time concern (today changes daily).
- **Absolute-date fallback beyond N days.** Rejected for parity simplicity (AC-cli always shows
  "in N days"); can be revisited if users ask.
