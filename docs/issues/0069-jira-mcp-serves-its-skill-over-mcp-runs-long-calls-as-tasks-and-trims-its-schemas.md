---
type: Issue
title: jira mcp serves its skill over MCP, runs long calls as tasks and trims its schemas
description: Serve the jira-ticket skill over the skills extension, run downloads and large searches as MCP tasks, and cut the tools/list context cost (3087 bytes on 0.2.2).
status: in-progress
timestamp: 2026-11-25T12:10:43Z
---

## 0069. jira mcp serves its skill over MCP, runs long calls as tasks and trims its schemas

### Problem

`jira mcp` publishes three tools whose `tools/list` costs 3087 bytes (about 772 tokens), sends no server `instructions`, serves no resources, and answers `skills/list` with an error (measured 2026-11-25 on 0.2.2 by sending `initialize`, `tools/list`, `resources/list` and `skills/list` to the binary). An agent connected over MCP never sees the `jira-ticket` skill, and a long attachment download or a large search blocks the call with no progress and no way to cancel it. Owner's request, 2026-11-25: every MCP server supports skills over MCP and MCP tasks, and cuts its context cost; the reference is qa-walkthrough-cli ADR 0063 and `crates/cli/src/mcp/`.

### Scope

Included: trimming repeated text from the published tool schemas; serving the `jira-ticket` skill over the skills extension; running long calls as tasks. Excluded: per-file download progress (the download core reports no events), new tools, release.

### Acceptance

- AC1: `tools/list` is smaller than 3087 bytes, and every tool, operation and field the schemas offered before is still offered. (`verify_by: test`)
- AC2: `initialize` declares the skills extension and sends instructions naming `skill://jira-ticket/SKILL.md`; `resources/list`, `resources/read`, `skills/list` and `skills/get` serve exactly the text `jira skill jira-ticket` prints. (`verify_by: test`)
- AC3: with the tasks extension declared by the client, `jira_download_attachments` and a `search` whose `limit` is above 50 return a task that `tasks/get` completes with the synchronous result and `tasks/cancel` ends as cancelled; every other call, and every call from a client without the extension, answers synchronously as before. (`verify_by: test`)

### Decision

- The MCP surface (skills resource, instructions, task rule, trimmed schemas) is decided in [ADR 0034](../adr/0034-jira-mcp-serves-the-skill-over-the-skills-extension-and-runs-long-calls-as-tasks.md).
- Slices run in sequence in one worktree: all three change `src/mcp/mod.rs` (session, 2026-11-25).
- The skill frontmatter is read by its two `key: value` lines (`name`, `description`), not by a YAML parser, since the embedded file has only those two plain keys; a missing key fails at startup (session, 2026-11-25).

### Plan

- S1: the published tool schemas drop repeated text and stay complete | files: src/mcp/mod.rs, tests/unit/mcp.rs | after: none
- S2: the server serves the `jira-ticket` skill over the skills extension and names it in its instructions | files: src/mcp/mod.rs, src/mcp/skills.rs, tests/unit/mcp.rs, tests/mcp_stdio.rs | after: S1
- S3: long calls become tasks for a client that declares the tasks extension, with progress heartbeats and cancellation | files: src/mcp/mod.rs, src/mcp/tasks.rs, tests/unit/mcp.rs, tests/mcp_stdio.rs | after: S2

### Outcome

Delivered on branch feat/ek-mcp-skills-tasks (commits 964f625 schema trim, fa1b288 skills extension, d2dc5fe tasks, d4d49dc review fixes), pending merge. Measured by sending JSON-RPC to the built binary: `tools/list` 3087 → 2549 bytes (about 772 → 637 tokens); `initialize` 141 → 369 bytes, now carrying the instructions and the skills and tasks extensions; `resources/list` (879 bytes) and `skills/list` (1014 bytes) are read on demand. A structural comparison of 0.2.2 and the new `tools/list` without descriptions is identical. Proof: `cargo test --test mcp_stdio` 25 passed three times in a row, each new test red under a planted mutation; full suite serial and parallel, clippy, fmt, comment policy and the skill stub check green. A fresh-context review requested changes (one test that could not fail, untested cancellation and direct path, docs); the fixes were approved by a narrow re-review.

The MCP section of `.claude/skills/jira-ticket/SKILL.md` now says that a client supporting MCP tasks gets a task for a download and for a search above `limit` 50; `scripts/check-skill-stubs.sh`, the skill tests and `cargo test --test mcp_stdio` stay green. Task TTL stays unbounded (as the reference), accepted.
