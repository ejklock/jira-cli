---
type: Issue
title: "C2 — non-interactive `jira comment` command (-m or stdin body, --json write result)"
description: One-shot Command::Comment posting through the C1 seam; issue from explicit key or current git branch; body from -m or piped stdin (TTY-guarded); localized confirmation or minified {"ok":true,"comment_id","issue_key"} via agent_json::comment_result; exit codes 0/2/1 with no false success and the E2 re-auth message on 401.
status: done
labels: [cli, write, comments, agent, parity]
blocked_by: 0045
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## C2 — non-TTY `jira comment`

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-C3 per
[ADR 0023](/adr/0023-non-tty-comment-command.md), behaviors
[BDR 0014](/bdr/0014-non-interactive-comment-behaviors.md) S1–S8.

Scope: `src/cli.rs` (Command::Comment), `src/main.rs` (dispatch + stdin
wiring), `src/commands.rs` (`comment_core`), `src/agent_json.rs`
(`comment_result`), `tests/unit/commands.rs`, `locales/pt_BR.json`.

Delivered without an `--instance` flag (the single-instance
`resolve_single_instance(None)` idiom jira-cli's other commands share) and
with `KNOWN_COMMANDS` gaining `comment` so `normalize_argv` does not prepend
`get`. Tests follow the wiremock `*_core` idiom (real client against a mock
server).
