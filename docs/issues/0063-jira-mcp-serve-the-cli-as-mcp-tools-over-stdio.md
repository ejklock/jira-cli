---
type: Issue
title: "jira mcp: serve the CLI as MCP tools over stdio"
description: Add jira mcp, an rmcp stdio server with jira_read, jira_write and jira_download_attachments as adapters over the command cores, flat schemas, WAL store.
status: closed
timestamp: 2026-11-14T15:37:08Z
---

## 0063. jira mcp: serve the CLI as MCP tools over stdio

### Problem

Agent sessions call `jira` through shell commands; the owner wants MCP tools. Part of the owner's request of 2026-11-14 to replicate in jira-cli what active-collab-cli did for discoverability and agent distribution. Scope chosen by the owner on 2026-11-14: README and metadata, agent distribution and a release. Not taken: the history purge (nothing to remove: every commit, message, PR, release and issue checked clean on 2026-11-14). Implements [ADR 0031](/adr/0031-jira-mcp-serves-the-cli-as-three-mcp-tools-over-stdio.md).

### Scope

Included: `jira mcp` subcommand; the three tools of ADR 0031 with their operations; the store in WAL mode; `docs/architecture.md` module and data-flow diagrams gain the MCP adapter; the `jira-ticket` skill documents `jira mcp`; `jira mcp --help` names the three tools.

Excluded: HTTP transport, MCP resources and prompts, `browse`, `setup`, transitions, comment edit and delete.

### Decision

- Three tools with flat schemas, mirroring active-collab-cli's final design (its issues 0073, 0080, 0091); see ADR 0031.
- Tests run only against the mocked Jira API the suite already uses; no check touches the owner's real Jira instance.

### Acceptance

- AC1: `tools/list` returns the three tools with their annotations; every `inputSchema` has top-level `type: object` and no top-level `oneOf`, `anyOf` or `allOf`. (`verify_by: test`)
- AC2: the `operation` enum of each tool lists exactly its operation variants, and every field a variant accepts appears in the flat schema. (`verify_by: test`)
- AC3: each operation's result is byte-identical to the matching `--json` command against the same mocked API. (`verify_by: test`)
- AC4: a core failure (bad ref, 404, 401) returns `isError` with the CLI's error text, and the server keeps serving; an unknown operation, a missing required field or a field the operation does not take is `invalid_params` and writes nothing. (`verify_by: test`)
- AC5: across every operation, stdout carries only JSON-RPC frames. (`verify_by: test`)
- AC6: the store opens in WAL mode, and two `jira mcp` processes run tool calls at the same time without a lock error. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Outcome

Delivered in PR #5 (merge fb9ca85): commits for the MCP server, the expect() removal and the review fixes. Proof: `cargo test -- --test-threads=1` 1039 passed; stdio tests compare every tool result byte for byte with the CLI `--json`; CI green.
