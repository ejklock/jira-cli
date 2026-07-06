---
type: Issue
title: "E2 — actionable 401 re-auth messaging (CLI + TUI status line)"
description: Typed ClientError::Unauthorized{instance} at the single gouqi wrapper boundary; CLI get/current/mine/search print the translate-then-substitute re-auth template (instance + `jira setup add`) to stderr with non-zero exit; TUI fetch spawn sites surface the same guidance on the D4 status row. Port of fork-base RA1–RA3.
status: done
labels: [cli, tui, errors, auth, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## E2 — 401 re-auth messaging

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-E2 (fork-base RA1–RA3
end state). Typed matching only — `classify_error` in `src/client.rs` is the
single `gouqi::Error::Unauthorized → ClientError::Unauthorized{instance}`
mapping point; no string matching. CLI paths render the ADR 0006 template
(en + pt-BR); TUI `spawn_load_list/detail/more` map it to
`Msg::LoadFailed(reauth_message)` consumed by the D4 status row.

**Accepted deviation:** the initial pre-TUI fetch (`fetch_and_run`) keeps its
existing stderr contract (its literal shape is pinned by a pre-existing test).

**Note — E1 (bare TTY → mine) required no work:** `bare_no_command_action`
already implements and tests the fork-base C1 behavior.
