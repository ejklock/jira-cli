---
type: ADR
title: jira mcp serves the CLI as three MCP tools over stdio
description: jira mcp exposes jira_read, jira_write and jira_download_attachments over stdio as adapters over the command cores, with flat schemas and a WAL store.
owner: Evaldo Klock
status: Proposed
timestamp: 2026-11-14T15:36:30Z
---

# 0031. jira mcp serves the CLI as three MCP tools over stdio

## Context

The owner runs several agent sessions in parallel (Claude Code, Codex, OpenCode) and wants each to call `jira` as MCP tools instead of shell commands. The sibling project active-collab-cli solved the same need with `ac mcp`, and its later fixes showed two traps: MCP annotations are per tool, so a download inside a read-only tool lies about its hint; and Claude Code drops a tool whose `inputSchema` has a top-level `oneOf`.

## Decision

- `jira mcp` serves an MCP server over stdio, built on `rmcp`, spawned by each harness session.
- Three tools, each operation an adapter over the existing command core run with `json = true`:
  - `jira_read` (`readOnlyHint: true`): `get` (ref, `no_comments`, `refresh`), `current` (optional `branch`, else the git branch of the server's working directory), `mine` (`limit`), `search` (`jql`, `limit`).
  - `jira_write` (`destructiveHint: true`): `comment` (optional ref resolved from the branch, body).
  - `jira_download_attachments` (not read-only, not destructive, idempotent): ref and optional target directory.
- Every operation takes an optional `instance`.
- Each tool publishes a flat schema: top-level `type: object`, a required `operation` enum, every operation's field as an optional property whose description names the operations that take it. The server parses into typed operation enums with `deny_unknown_fields`; an unknown operation or a field the operation does not take is `invalid_params`.
- A non-zero core exit maps to `isError: true` with the core's error text; a 401 keeps the CLI's re-auth message. stdout carries only JSON-RPC frames.
- The SQLite store opens in WAL mode so parallel `jira mcp` processes share it without lock errors.

## Alternatives

- One tool per verb: more tools in every session's context, growing with each verb. Not taken.
- One tool for everything: loses the harness's separate approval of reads and writes. Not taken.
- Downloads as an operation of `jira_read`: its read-only hint would be false. Not taken.

## Consequences

- Each new CLI verb adds an operation to one of the three tools and its schema test.
- The flat schema and the operation enums are two declarations of one contract, kept equal by a test.
- Out of scope: HTTP transport, MCP resources and prompts, the `browse` TUI, `setup` (token entry stays interactive), transitions and comment edit/delete.
