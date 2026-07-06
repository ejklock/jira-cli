---
type: Issue
title: "D2 — browse list as per-issue cards with colored relative due date"
description: Replace the list Table with one bordered card per issue — line 1 KEY summary, line 2 relative colored due · status · project. Threads duedate + project into IssueRow from the search payload (serde default, no extra fetch).
status: done
labels: [tui, design, cards, parity]
blocked_by: [0030]
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## D2 — list cards + colored due

Implements [ADR 0014](/adr/0014-tui-visual-design-system.md) §3; behavior
[BDR 0007](/bdr/0007-tui-visual-design-behaviors.md) S2/S3/S4.

### Scope

- `IssueRow` gains `duedate: Option<String>` + `project: Option<String>`
  (serde default), mapped from the search payload — no extra fetch.
- Pure card line builder (line 1 `KEY summary`, line 2
  `{relative_due} · {status} · {project}`, omit-empty segments); due text via
  the existing `relative_due` (ADR 0013), color via `theme::due_style`.
- List render swaps the Table for stacked cards; selection styles the whole
  card; pagination/load-more and search behavior unchanged.

### Acceptance

- BDR 0007 S2, S3, S4 pass on `TestBackend`; card builder unit tests
  (mutation-sensitive full-line assertions, pt-BR labels via `t()`).
- Cached list snapshots (pre-field) still deserialize (serde default test).
- Suite green; clippy `--all-targets -D warnings`, fmt, comment-policy clean.
