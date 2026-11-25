---
type: ADR
title: jira mcp serves the skill over the skills extension and runs long calls as tasks
description: jira mcp serves the jira-ticket skill as skill://jira-ticket/SKILL.md over the skills extension, names it in its instructions, runs downloads and large searches as MCP tasks for clients that declare the extension, and trims its tool schemas.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-25T12:10:23Z
---

# 0034. jira mcp serves the skill over the skills extension and runs long calls as tasks

## Context

[ADR 0031](0031-jira-mcp-serves-the-cli-as-three-mcp-tools-over-stdio.md) defined `jira mcp` as three tools and nothing else: no instructions, no resources. The agent skill lives in the binary and is printed by `jira skill jira-ticket` ([ADR 0028](0028-agent-skill-served-by-jira-skill-command.md)), but a client connected over MCP never sees it. Attachment downloads and large searches can take long, and a synchronous tool call gives the client no progress and no cancellation. The owner asked on 2026-11-25 that every MCP server of ours serve its skill over MCP, run long calls as MCP tasks, and cut its context cost. qa-walkthrough-cli did the same in its ADR 0063, on the same `rmcp` 3.9.0, which implements the skills extension (SEP-2640) and the tasks extension `io.modelcontextprotocol/tasks` (SEP-2663) in its `server` feature.

## Decision

- The server declares the skills extension and serves the embedded `.claude/skills/jira-ticket/SKILL.md` as the resource `skill://jira-ticket/SKILL.md` (`text/markdown`), through `resources/list`, `resources/read`, `skills/list` and `skills/get`. The text is the same bytes `jira skill jira-ticket` prints; no request argument reaches the filesystem.
- The server sends `instructions` that name `skill://jira-ticket/SKILL.md` as the place to read the tool contract.
- The server declares the tasks extension. When the client declares it too, `jira_download_attachments` and a `jira_read` `search` with `limit` above 50 (`DEFAULT_SEARCH_LIMIT`) return a task; `tasks/get` returns the result the synchronous call would, and `tasks/cancel` stops the command and ends the task `cancelled`. Every other call, and every call from a client without the extension, stays synchronous.
- While a task-eligible call runs, a client that sent a progress token gets a heartbeat progress notification every 5 s. The cores report no per-file progress, so none is sent.
- The published schemas drop repeated text: no `$schema` key, no "Taken by" note on a tool with one operation. Tools, operations, fields and their parsing are unchanged.

## Alternatives

- Keep the skill CLI-only: agents on MCP-only clients never get the contract. Not taken.
- A `jira_skill` tool returning the text: costs a tool schema in every session instead of a resource read on demand. Not taken.
- Every call as a task: short reads would pay the task round trips. Not taken.
- Per-file download progress: needs an event channel out of `download_core`. Deferred.

## Consequences

- A client with the skills extension reads the contract from the server, so the plugin's skill stub and the MCP server agree by construction.
- Cancelling a download task may leave the file being written incomplete; files already saved stay.
- Server `instructions` add a few dozen tokens to each session; the trimmed schemas more than pay for them (measured in issue 0069).
