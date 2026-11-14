---
type: Issue
title: Install jira in Claude Code, Codex, OpenCode and Pi as a plugin
description: A jira-cli git-marketplace plugin for Claude Code and Codex and one jira-cli-agent npm package for OpenCode and Pi, each wiring jira mcp and the shared skill stub.
status: open
timestamp: 2026-11-14T15:37:09Z
---

## 0064. Install jira in Claude Code, Codex, OpenCode and Pi as a plugin

### Problem

Installing `jira` for an agent takes manual steps per harness. Part of the owner's request of 2026-11-14 to replicate in jira-cli what active-collab-cli did for discoverability and agent distribution. Scope chosen by the owner on 2026-11-14: README and metadata, agent distribution and a release. Not taken: the history purge (nothing to remove: every commit, message, PR, release and issue checked clean on 2026-11-14). Implements [ADR 0032](/adr/0032-agent-plugins-for-four-harnesses-ship-from-this-repository.md). Depends on [issue 0063](/issues/0063-jira-mcp-serve-the-cli-as-mcp-tools-over-stdio.md).

### Scope

Included: the `jira-cli` plugin under `plugins/jira-cli/` with both marketplace files; the `jira-cli-agent` npm package under `packages/jira-cli-agent/` with its README and `node --test` cases; the npm workspace root; a skill-stub check script with a test that plants drifts; a proof script that installs each plugin into a temporary home with the harness's own CLI; CI running the stub check, its test and `node --test`; README install steps per harness.

Excluded: `npm publish` (owner, 2FA), pushing, the release; bundling the binary; Copilot and Cursor.

### Decision

- Names `jira-cli` (plugin, MCP server) and `jira-cli-agent` (npm), owner 2026-11-14. Not taken: `jira-ticket` / `jira-ticket-agent`; `jira` / `jira-agent`.
- Layout and proof mirror active-collab-cli's `plugins/`, `packages/`, `scripts/check-skill-stubs.sh` and `scripts/prove-plugins.sh`.
- The proof never touches the owner's real harness configs: temporary `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, OpenCode config and Pi home only.

### Acceptance

- AC1: `claude plugin validate .` passes, and marketplace add plus `claude plugin install jira-cli@<marketplace>` from this checkout in a temporary `CLAUDE_CONFIG_DIR` lists the `jira-cli` MCP server and the `jira-ticket` skill. (`verify_by: command`)
- AC2: `codex plugin marketplace add` from this checkout in a temporary `CODEX_HOME` installs the plugin with the MCP server and skill. (`verify_by: command`)
- AC3: OpenCode, given `packages/jira-cli-agent` by local path in a temporary config, registers the `jira mcp` server and the skill. (`verify_by: command`)
- AC4: Pi installs `packages/jira-cli-agent` from its local path in a temporary home, lists the skill, and loads no extension from it. (`verify_by: command`)
- AC5: `npm pack --dry-run -w packages/jira-cli-agent` lists only `package.json`, `index.ts`, the skill and the README. (`verify_by: command`)
- AC6: every plugin and package skill copy equals the stub `install-skill.sh` writes, and every manifest name and version matches; the check's test fails on each planted drift. (`verify_by: test`)
- AC7: the README gives one install step per harness and says `jira` must have the `mcp` subcommand. (`verify_by: inspection`)
- CC: no superfluous comments, banners or commented-out code; the owner's real configs are never touched. (`verify_by: inspection`)

### Plan

- S1: Claude Code and Codex plugin `jira-cli`, both marketplace files, skill stub copy and its byte-equality/name/version check with a planted-drift test | files: plugins/jira-cli/**, .claude-plugin/marketplace.json, .agents/plugins/marketplace.json, scripts/check-skill-stubs.sh, scripts/test-check-skill-stubs.sh | after: issue 0063
- S2: npm workspace root and the `jira-cli-agent` package (OpenCode config hook, Pi skill via the `pi` key, README, node --test cases) | files: package.json, package-lock.json, packages/jira-cli-agent/** | after: S1
- S3: proof script installing each plugin in a temporary home with each harness's CLI, CI wiring for the stub check, its test and node --test, README install step per harness | files: scripts/prove-plugins.sh, .github/workflows/ci.yml, README.md | after: S2
