---
type: Issue
title: Make the README and package metadata easy to find
description: README opens with what jira is, badges and a feature list, and the quickstart gains the plugin step; Cargo.toml and the GitHub repo carry description, license, keywords and topics.
status: open
timestamp: 2026-11-14T15:37:10Z
---

## 0065. Make the README and package metadata easy to find

### Problem

Part of the owner's request of 2026-11-14 to replicate in jira-cli what active-collab-cli did for discoverability and agent distribution. Scope chosen by the owner on 2026-11-14: README and metadata, agent distribution and a release. Not taken: the history purge (nothing to remove: every commit, message, PR, release and issue checked clean on 2026-11-14). The README lacks badges and a feature list; `Cargo.toml` has no `description`, `license`, `repository`, `homepage`, `keywords` or `categories`; the GitHub repository has no topics and no homepage (checked with `gh repo view` on 2026-11-14). Mirrors active-collab-cli PR #45. Depends on [issue 0064](/issues/0064-install-jira-in-claude-code-codex-opencode-and-pi-as-a-plugin.md) for the plugin section.

### Scope

Included: README title, badges (CI, release, license), feature list, quickstart plugin step, one agent-install table; `Cargo.toml` metadata; npm package and Claude plugin descriptions and keywords; GitHub description, topics and homepage through `gh repo edit` (outward-facing, run after the owner approves the text).

Excluded: the unofficial notice text, which stays as is.

### Acceptance

- AC1: the README's first screen says what `jira` is, shows the badges and the feature list, and the quickstart includes the plugin step. (`verify_by: inspection`)
- AC2: `cargo metadata` shows description, license MIT, repository, readme, keywords and categories; no `homepage`, which cargo reports as redundant with `repository` when both name the repository. (`verify_by: command`)
- AC3: `gh repo view --json description,repositoryTopics,homepageUrl` shows the approved values. (`verify_by: command`)
- CC: outward text names no internal tool. (`verify_by: inspection`)

### Decision

- GitHub description "Unofficial Jira Cloud CLI, terminal UI and MCP server: read issues, post comments and download attachments from the terminal or an AI coding agent.", topics jira, jira-cloud, cli, tui, mcp, model-context-protocol, ai-agent, claude-code, codex, opencode, rust, sqlite, and an empty homepage (owner, 2026-11-14). Not taken: description only; no change.
