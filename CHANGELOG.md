# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
