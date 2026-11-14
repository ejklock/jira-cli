---
type: Issue
title: Release 0.2.0 with the MCP server and agent plugins
description: Bump Cargo.toml and every plugin and package manifest to 0.2.0, close the Unreleased changelog and tag v0.2.0; fixes the binary reporting 0.1.0 since v0.1.1.
status: open
timestamp: 2026-11-14T15:37:11Z
---

## 0066. Release 0.2.0 with the MCP server and agent plugins

### Problem

Part of the owner's request of 2026-11-14 to replicate in jira-cli what active-collab-cli did for discoverability and agent distribution. Scope chosen by the owner on 2026-11-14: README and metadata, agent distribution and a release. Not taken: the history purge (nothing to remove: every commit, message, PR, release and issue checked clean on 2026-11-14). Separate defect found on 2026-11-14: `Cargo.toml` still says `version = "0.1.0"` while the tags reached `v0.1.2` (`git show v0.1.2:Cargo.toml`), so `jira --version` of the v0.1.1 and v0.1.2 binaries reports 0.1.0.

### Scope

Included: version 0.2.0 in `Cargo.toml`, `Cargo.lock`, plugin, marketplace and npm manifests; `CHANGELOG.md` `[Unreleased]` becomes `[0.2.0]` with the MCP server, the plugins and the earlier skill rename; a release note that the store now opens in WAL, so a pre-0.2.0 binary run at the same time as a 0.2.0 `jira mcp` may report "database is locked" on its journal-mode switch (reviewer of issue 0063, inference, not reproduced), and runs normally once alone; the tag `v0.2.0` pushed by name after the owner's word.

Excluded: `npm publish` (owner, 2FA).

### Decision

- Version 0.2.0 (owner, 2026-11-14): MCP and plugins are features, and the skill rename breaks `jira skill jira`. Not taken: 0.1.3.

### Acceptance

- AC1: `jira --version` of the release build prints 0.2.0. (`verify_by: command`)
- AC2: the stub check passes with every manifest at 0.2.0. (`verify_by: command`)
- AC3: the release workflow of `v0.2.0` publishes every target's binary. (`verify_by: command`)
