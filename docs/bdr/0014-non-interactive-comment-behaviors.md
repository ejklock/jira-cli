---
type: BDR
title: "`jira comment` posts a comment to an issue as the logged-in user, from -m or stdin, with a --json write result"
description: Running `jira comment [ISSUE_KEY] [-m TEXT] [--json]` posts a comment to the resolved issue (explicit key, or the current git branch's key) as the logged-in user. Body from -m or piped stdin (multi-line verbatim); empty body or unresolvable issue is a usage error (exit 2) with no write. Success prints a localized confirmation or, with --json, one minified {"ok":true,"comment_id":"...","issue_key":"..."}; failures exit non-zero with no false success; 401 surfaces the re-auth message.
status: Accepted
superseded_by:
supersedes:
tags: [cli, comments, write, agent, json, non-interactive]
timestamp: 2026-07-07T00:00:00Z
---

# 0014. Non-interactive comment creation

## Context

The C1 seam ([ADR 0022](/adr/0022-comment-write-seam.md)) can write comments
but nothing user-facing consumes it. This BDR specifies the observable
behavior of the one-shot `comment` command
([ADR 0023](/adr/0023-non-tty-comment-command.md)), the Jira adaptation of
the fork base's BDR 0027.

## Textual Description

Running `jira comment`:

- The **issue** is the explicit `ISSUE_KEY`; when omitted, it is resolved
  from the **current git branch** (as `current` does). Neither resolves →
  error, exit 2, **no write**.
- The **body** is `-m/--message <TEXT>` when given; otherwise read in full
  from **piped stdin** (multi-line preserved). With a TTY stdin and no `-m`,
  or an empty body: `no comment body`, **exit 2**, nothing posted.
- The comment is posted as the **logged-in user** (the instance's Basic-auth
  credentials); with no configured instance the command errors ("not logged
  in") without writing. There is no way to post as another user.
- **Success:** without `--json`, one localized confirmation line; with
  `--json`, exactly one minified line
  `{"ok":true,"comment_id":"<id>","issue_key":"<KEY>"}`. Exit `0`.
- **Failure** (no body, no issue, no instance, HTTP 4xx/5xx): non-zero exit,
  error on stderr — or with `--json` one minified
  `{"ok":false,"error":"<reason>"}`. **No false success.** A 401 surfaces
  the standard re-auth message (BDR/E2 contract).

## Scenarios

**Scenario 1: post via -m on an explicit key** — Given a configured
instance, When the user runs `jira comment PROJ-42 -m "Deploy em homolog."`,
Then the comment is created on PROJ-42 as the logged-in user, a confirmation
prints, exit 0.

**Scenario 2: post via stdin pipe, multi-line** — Given a configured
instance, When the user runs `printf 'Linha 1\nLinha 2' | jira comment
PROJ-42`, Then the two-line body is posted verbatim (rendered with a
hardBreak), exit 0.

**Scenario 3: --json write result** — Given a configured instance, When the
user runs `jira comment PROJ-42 -m "ok" --json`, Then stdout is exactly one
minified line `{"ok":true,"comment_id":"<id>","issue_key":"PROJ-42"}`,
exit 0.

**Scenario 4: empty body is a usage error** — Given no `-m` and a TTY (or
empty) stdin, When the user runs `jira comment PROJ-42`, Then the command
reports `no comment body`, exits 2, and **no** `add_comment` call is made.

**Scenario 5: issue from the current branch** — Given the working directory
is on a git branch containing an issue key and no `ISSUE_KEY` argument, When
the user runs `jira comment -m "..."`, Then the comment is posted to that
branch's issue, exit 0.

**Scenario 6: no issue resolvable** — Given no `ISSUE_KEY` and a branch with
no key, When the user runs `jira comment -m "..."`, Then the command errors,
exits 2, no write.

**Scenario 7: not logged in** — Given no configured instance, When the user
runs `jira comment PROJ-42 -m "..."`, Then the command errors ("not logged
in"), exits non-zero, no write.

**Scenario 8: HTTP failure is not a false success** — Given the server
returns 4xx/5xx, When the user posts, Then the exit is non-zero and the
failure is reported (stderr, or the `--json` error object); a 401 prints the
re-auth message. Never a success line.

## Test Design

`comment_core` is unit-tested against the mocked `JiraClient` with injected
writers and an injected body source (flag vs stdin text vs tty-marker); the
branch resolution reuses the `current` extraction (already covered) and is
asserted through the mock's received key. The wiremock write-path and token
host behavior are C1's tests (not re-proven here).

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Flag body, explicit key | unit | 1 | `add_comment("PROJ-42", "Deploy…")` called once; confirmation on stdout; exit 0 | happy path |
| Stdin body multi-line | unit | 2 | body passed verbatim incl. `\n`; exit 0 | stdin channel |
| `--json` success | unit | 3 | stdout is one minified ok-object with comment_id + issue_key; nothing else | write-result contract |
| Empty body / TTY stdin | unit | 4 | exit 2; `no comment body`; `add_comment` NOT called | usage guard |
| Branch-resolved key | unit | 5 | with no arg, the branch's key reaches the mock | current-branch fallback |
| No issue resolvable | unit | 6 | exit 2; no `add_comment` | safe resolution failure |
| No instance | unit | 7 | non-zero; "not logged in"; no client construction | login required |
| HTTP failure / 401 | unit | 8 | non-zero; `{"ok":false,…}` with --json; 401 → re-auth message; no success line | no false success |

## Related

- ADR: [/adr/0023-non-tty-comment-command.md](/adr/0023-non-tty-comment-command.md)
- ADR: [/adr/0022-comment-write-seam.md](/adr/0022-comment-write-seam.md), [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md)
- Issue: [/issues/0046-c2-non-tty-comment-command.md](/issues/0046-c2-non-tty-comment-command.md)
- Fork base: `active-collab-cli` BDR 0027
