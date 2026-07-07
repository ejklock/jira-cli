---
type: ADR
title: "A non-interactive `comment` command posts a comment as the logged-in user, extending the agent --json contract to a write result"
description: Add a one-shot `jira comment [ISSUE_KEY] [-m TEXT] [--json]` subcommand (parallel to get/current/mine) that resolves the issue from the explicit key or the current git branch, takes the body from -m or piped stdin, and posts it through the C1 write seam (client.add_comment). --json emits a curated minified write result {"ok":true,"comment_id":"...","issue_key":"..."}; failures exit non-zero with no false success. Identity is structural: the instance's Basic-auth credentials attribute the comment to the logged-in user.
status: Accepted
supersedes:
superseded_by:
tags: [cli, comments, write, agent, json, non-interactive, llm]
timestamp: 2026-07-07T00:00:00Z
---

# 0023. Non-interactive `comment` command

## Context

C1 ([ADR 0022](/adr/0022-comment-write-seam.md)) landed the write seam;
nothing consumes it yet. The parity program
([PRD 0003](/prd/0003-active-collab-parity.md) R-C3, fork base ADR 0040 /
BDR 0027) requires a non-TTY path so an LLM/agent/script can comment without
the TUI. The CLI already has every supporting piece: the `current` command's
branch→issue-key resolution, the instance resolution `get` uses, the curated
agent `--json` read contract ([ADR 0004](/adr/0004-agent-json-output-contract.md)),
and the `*_core` decomposition pattern (injected writers, mocked client).

## Decision

1. **Invocation:** `jira comment [ISSUE_KEY] [-m|--message <TEXT>] [--json]`
   — `Command::Comment` in `src/cli.rs`, dispatched parallel to `get`.
   `ISSUE_KEY` optional: when omitted, resolve from the current git branch
   with the same extraction `current` uses. No key from either source →
   usage error, exit 2, **no write**.
2. **Body channel:** `-m` wins; otherwise read stdin to EOF (multi-line
   verbatim) — but **only when stdin is not a TTY** (`std::io::IsTerminal`),
   so an interactive invocation without `-m` fails fast (exit 2,
   `no comment body`) instead of hanging. Empty body → exit 2, no write.
3. **Write result contract (extends ADR 0004 to a write):** with `--json`,
   exactly one minified line — success
   `{"ok":true,"comment_id":"<id>","issue_key":"<KEY>"}` (Jira comment ids
   are strings; the fork's numeric `task_id`/`project_id` pair collapses to
   `issue_key`), failure `{"ok":false,"error":"<reason>"}` + non-zero exit.
   `agent_json::comment_result` owns the shape. Without `--json`, one
   localized human confirmation line (`t()`, pt-BR catalog).
4. **Identity is structural.** The post rides the picked instance's
   Basic-auth client (C1 seam, host-pinned) — the comment is attributed to
   the token owner. No configured instance → "not logged in", non-zero, no
   write. No impersonation flag exists.
5. **Exit codes mirror get/current:** `0` posted; `2` usage (no body / no
   resolvable issue); `1` runtime (no instance, HTTP 4xx/5xx — never a
   false success; 401 surfaces the R-E2 re-auth message).
6. **Decomposition:** `comment_core(args, body_source, client, out, err) ->
   exit_code` mirrors `get_core` — unit-tested against a mocked client and
   injected writers; `dispatch` does the thin I/O wiring (instance, stdin).
   One write path: it calls the same `client.add_comment` the TUI compose
   (C3) will use — a deletion test, not a second implementation.

## Alternatives considered

- **Piping into the TUI.** Rejected — agents need a one-shot command with a
  deterministic exit code and parseable output.
- **`-m` only, no stdin.** Rejected — multi-line bodies via shell escaping
  are hostile to LLMs; stdin is the natural non-TTY channel.
- **Reusing the fork's `{task_id, project_id}` result fields.** Rejected —
  Jira's identity is the issue key; inventing numeric ids would fake parity.

## Consequences

**Positive:** first write consumer lands; agents get
`printf '...' | jira comment PROJ-1 --json` with a stable contract.
**Trade-offs:** the write-result shape is a new contract agents will depend
on ([BDR 0014](/bdr/0014-non-interactive-comment-behaviors.md)); no
server-truth refresh here (a one-shot CLI has no thread to refresh — ADR
0015 §4 binds the TUI slices).

## Related

- ADR: [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md) (the seam consumed)
- ADR: [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md), [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md)
- BDR: [/bdr/0014-non-interactive-comment-behaviors.md](/bdr/0014-non-interactive-comment-behaviors.md)
- Issue: [/issues/0046-c2-non-tty-comment-command.md](/issues/0046-c2-non-tty-comment-command.md)
- Fork base: `active-collab-cli` ADR 0040 / BDR 0027
