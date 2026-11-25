# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-11-25

### Changed

- `jira skill jira-ticket` now tells the agent to use the `jira` MCP tools when
  that server is connected, and to run the CLI with `--json` otherwise

## [0.2.0] - 2026-11-14

### Upgrade note

The store now opens in WAL mode. A `jira` older than 0.2.0 running at the same
time as a 0.2.0 `jira mcp` may report "database is locked" when it switches
the journal mode; it runs normally once it is the only process using the store.

### Changed

- Renamed the agent skill identifier from `jira` to `jira-ticket`
  ([ADR 0030](docs/adr/0030-rename-agent-skill-to-jira-ticket.md)). `jira
  skill jira-ticket` prints the contract; `jira skill jira` now exits 2
  (unknown skill) — there is no back-compat alias. Existing installs must
  re-run `install-skill.sh --force` to rewrite stub paths under
  `skills/jira-ticket/` (`.cursor/rules/jira-ticket.mdc` for Cursor).
- `install-skill.sh` now removes a stale pre-rename `jira` skill stub when
  it finds one, for both project and global installs. It only removes
  files containing the thin-pointer marker, so an unrelated skill named
  `jira` stays untouched.

### Added

- Agent plugins: the `jira-cli` plugin for Claude Code and Codex (installed
  from this repository's marketplace) and the `jira-cli-agent` npm package for
  OpenCode and Pi, each giving the `jira mcp` tools and the `jira-ticket`
  skill (Pi: the skill). They need `jira` 0.2.0 or later.
  `scripts/prove-plugins.sh` installs each one in a temporary home to prove it.
- `jira mcp` serves the CLI as an MCP server over stdio
  ([ADR 0031](docs/adr/0031-jira-mcp-serves-the-cli-as-three-mcp-tools-over-stdio.md)).
  Three tools, each operation an adapter over the existing `--json` command
  core: `jira_read` (`get`, `current`, `mine`, `search`), `jira_write`
  (`comment`) and `jira_download_attachments` (`download`). Tool results are
  byte-identical to the matching `--json` output; a core failure is a tool
  result with `isError`, and a malformed call is an `invalid_params` error.
- The SQLite store opens in WAL mode with a busy timeout, so parallel
  `jira mcp` processes (one per agent session) share one database without lock
  errors. An existing database switches to WAL on its first open.

- Sandbox note in the canonical `SKILL.md`: documents the `JIRA_DB`
  environment override for a read-only config directory, plus the
  copy-to-writable-path workaround.
- The skill body now documents `--download-attachments` and
  `--download-dir`, including fail-fast semantics: one failed download
  aborts the whole request and exits non-zero. The separate `--json`
  result object is `{issue_key, saved[]}`, not the issue schema.

## [0.1.0] - 2026-06-29

### Added

- Jira Cloud read CLI: `setup`, `get`, `current`, `mine`/`list`, `search` commands.
- Multi-instance configuration with per-instance Jira Cloud base URL and credentials.
- Local-first SQLite cache for fast offline reads.
- Interactive `browse` TUI: project list, issue detail, ADF content rendering with tables, attachments, comments.
- Comment write operations: create, edit, delete comments on issues.
- Status transitions: read available transitions and execute transitions requiring no screen fields.
- Internationalization: English and Brazilian Portuguese translations embedded at compile time.
- Attachment download with `--download-attachments` and external viewer support.
- `agent_json` output contract for structured LLM/agent consumption via `--json` flag.
- Agent skill (`jira skill jira`) and multi-harness installer (`install-skill.sh`) for Claude Code, pi, Codex CLI, OpenCode, GitHub Copilot, and Cursor.
- Bash and PowerShell release installers (`install.sh`, `install.ps1`) that fetch pre-built binaries from GitHub Releases.
- Bare-invocation routing: `jira` runs `mine` on TTY or shows help on non-TTY; `jira PROJ-123` runs `get`.
