---
type: ADR
title: Agent plugins for four harnesses ship from this repository
description: A jira-cli git-marketplace plugin for Claude Code and Codex and one jira-cli-agent npm package for OpenCode and Pi wire jira mcp and the skill stub.
owner: Evaldo Klock
status: Proposed
timestamp: 2026-11-14T15:36:32Z
---

# 0032. Agent plugins for four harnesses ship from this repository

## Context

Installing `jira` for an agent takes manual steps per harness: `claude mcp add` for the MCP server and `install-skill.sh` for the skill stub. Claude Code, Codex, OpenCode and Pi each have a plugin or package mechanism that can do both in one step. The sibling project active-collab-cli ships the same four from its own repository (git marketplace plus one npm package).

## Decision

- Plugins live in this repository, in an npm workspace, mirroring active-collab-cli.
- Claude Code and Codex install a plugin named `jira-cli` from a git marketplace in this repository (`.claude-plugin/marketplace.json`, `.agents/plugins/marketplace.json`). The plugin wires the `jira-cli` MCP server (`jira mcp`) through one root `mcp.json` named by the Claude manifest, keeps a root `plugin.json` for Codex, and ships the `jira-ticket` skill stub.
- OpenCode and Pi install one npm package, `jira-cli-agent`: OpenCode loads its `config` hook (registers `jira mcp` and the skills path; a user's own MCP entry wins); Pi loads only the skill its `pi` key lists.
- The skill stub exists in `install-skill.sh` and in the plugin and package; a check script keeps the copies byte-equal and every manifest name and version equal to `Cargo.toml`.
- Names `jira-cli` / `jira-cli-agent` chosen by the owner on 2026-11-14. Not taken: `jira-ticket` / `jira-ticket-agent` (the skill name), `jira` / `jira-agent`.
- Manifests name the author Evaldo Klock and license MIT, as `LICENSE` does.
- The binary is not bundled: the plugins require `jira` with the `mcp` subcommand on PATH.

## Alternatives

- Keep `install-skill.sh` plus `claude mcp add` only: several manual steps per harness. Not taken.
- A separate plugins repository: two places to version together. Not taken.
- Two npm packages, one per harness: users must learn which name their harness takes. Not taken (active-collab-cli already merged its two into one).

## Consequences

- Every release bumps the plugin, marketplace and npm manifests with `Cargo.toml`.
- `npm publish` runs with the owner's 2FA, outside this repository's automation.
- se-configs installs the skill through `install-skill.sh`; a switch to the plugin there is se-configs' change.
