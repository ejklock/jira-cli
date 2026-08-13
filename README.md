# jira-cli

**Unofficial** command-line tool and interactive terminal UI (TUI) for reading
and browsing [Jira Cloud](https://www.atlassian.com/software/jira) issues.
Supports multi-instance configuration, local-first SQLite cache, and outputs
human-readable or JSON issue views.

The application ships as a single self-contained binary (`jira`) built with
Rust. No interpreter or runtime is required on the target.

> ## Unofficial — not affiliated with Atlassian
>
> This is an independent, community-built project. It is **not** an official
> Atlassian product and is **not affiliated with, endorsed by, sponsored by,
> or supported by** Atlassian. **"Jira" and "Atlassian" are trademarks of
> their respective owners** and are used here **only** to describe compatibility
> with the Jira Cloud REST API. This tool stores no credentials beyond a local
> API token, sends that token only to your configured host, and is provided
> "as is", without warranty. Use at your own risk.

---

## Quickstart

From zero to your first issue in three steps:

```sh
# 1. Install (macOS / Linux — Windows: see the PowerShell one-liner below)
curl -fsSL https://raw.githubusercontent.com/ejklock/jira-cli/main/install.sh | sh

# 2. Register your Jira Cloud instance.
#    Prompts once for an API token — the token is stored locally in SQLite.
jira setup add --name work --url https://yourorg.atlassian.net --email you@example.com

# 3. Fetch an issue
jira get PROJ-123
```

Then, day to day:

```sh
jira                    # issues assigned to you (TTY) or help (non-TTY)
jira get PROJ-123       # a specific issue by key or full URL
jira current            # the issue for your current git branch
jira mine --json        # machine-readable output for scripts and agents
```

---

## Install

### macOS / Linux (curl one-liner)

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/jira-cli/main/install.sh | sh
```

The script downloads the pre-built `jira` binary for your platform from the
latest GitHub Release and places it on your PATH.

### Windows (PowerShell one-liner)

```powershell
irm https://raw.githubusercontent.com/ejklock/jira-cli/main/install.ps1 | iex
```

Installs `jira.exe` into `%LOCALAPPDATA%\Programs\jira` and adds it to your
user PATH.

### Manual download

Download the pre-built binary for your platform from the
[Releases page](https://github.com/ejklock/jira-cli/releases), place it on your
PATH, and make it executable (`chmod +x jira` on Unix).

| Platform | Asset |
|---|---|
| Linux x86\_64 | `jira-linux-x86_64` |
| macOS x86\_64 (Intel) | `jira-macos-x86_64` |
| macOS arm64 (Apple Silicon) | `jira-macos-arm64` |
| Windows x86\_64 | `jira-windows-x86_64.exe` |

### Build from source (Docker required)

No local Rust toolchain needed. The crate is at the repo root; Docker provides
the build environment.

```sh
# Development build
docker compose run --rm dev cargo build

# Release binary (placed in target/release/jira)
docker compose build
docker compose run --rm build
```

---

## Commands

### setup — manage instances

```sh
# Register a Jira Cloud instance (interactive wizard prompts for missing fields)
jira setup add
jira setup add --name work --url https://yourorg.atlassian.net --email me@example.com
# API token is always entered hidden via a prompt — never passed as a flag.

# List configured instances (tokens never shown)
jira setup list

# Remove an instance and its cached issues
jira setup remove --name work

# Test connectivity to all (or one) configured instance
jira setup test
jira setup test --name work

# Show the current display language
jira setup language

# Set the display language (persists to SQLite; survives across invocations)
jira setup language en
jira setup language pt-BR
```

### get — fetch an issue by key or URL

```sh
jira get PROJ-123
jira get https://yourorg.atlassian.net/browse/PROJ-123
```

### current — fetch the issue from the current git branch

Branch must contain a valid Jira issue key (e.g. `PROJ-123`, `feature/PROJ-123`).

```sh
jira current
```

### mine — list open issues assigned to you

```sh
jira mine
jira list          # alias
```

When run in a terminal (TTY), `mine` opens an interactive arrow-key list of
your open issues. When output is piped or redirected (non-TTY), it falls back
to a plain table suitable for scripts.

### search — search for issues with JQL

```sh
jira search "assignee = currentUser() AND status = In Progress"
```

### browse — interactive TUI

Arrow-key terminal browser for your open issues. Navigate projects, view
issue detail, attachments, and comments.

```sh
jira browse
```

### comment — post a comment to an issue

```sh
jira comment PROJ-123 -m "Investigating now."
# or pipe a multi-line comment from stdin:
echo "Full investigation notes" | jira comment PROJ-123
```

When invoked without an issue key, `jira comment` resolves the key from the
current git branch.

### skill — print the agent skill contract

```sh
jira skill jira-ticket   # print the full --json read contract
```

---

## Authentication

`jira-cli` supports **Jira Cloud only** via Basic auth with an Atlassian API
token. The token is stored locally in a SQLite database. The password is
never stored. See the [Atlassian documentation](https://id.atlassian.com/manage-profile/security/api-tokens)
for generating API tokens.

---

## Internationalization

The binary ships with English (default) and Brazilian Portuguese (`pt-BR`)
translations for all user-facing output. Translations are embedded at compile
time — no external files required at runtime.

**Durable setting** — persist your preferred language to SQLite:

```sh
jira setup language pt-BR   # set
jira setup language          # show current
```

**One-off override** — the `JIRA_CLI_LANG` environment variable overrides the
stored setting for a single invocation:

```sh
JIRA_CLI_LANG=pt-BR jira browse
```

**Resolution order:** `JIRA_CLI_LANG` env var → SQLite setting → `en`.

---

## Agent skill

`jira` ships a self-describing **agent skill** for the `--json` read contract,
so an LLM coding agent can learn how to read your Jira issues non-interactively.
The full contract lives in **one place** — inside the binary — and `jira` prints
it on demand:

```sh
jira skill jira-ticket   # print the full jira-ticket contract
```

### Install the skill into your agent harness

Run the installer from your project root:

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/jira-cli/main/install-skill.sh | sh -s -- --harness all
```

| Harness | Project file (`--scope project`, default) | User-level file (`--scope global`) |
|---|---|---|
| Claude Code | `.claude/skills/jira-ticket/SKILL.md` | `~/.claude/skills/jira-ticket/SKILL.md` |
| pi | `.pi/skills/jira-ticket/SKILL.md` | `~/.pi/agent/skills/jira-ticket/SKILL.md` |
| Codex CLI | `.codex/skills/jira-ticket/SKILL.md` | `~/.codex/skills/jira-ticket/SKILL.md` |
| OpenCode | `.opencode/skills/jira-ticket/SKILL.md` | — (install per-project) |
| GitHub Copilot | `.github/skills/jira-ticket/SKILL.md` | — (install per-project) |
| Cursor | `.cursor/rules/jira-ticket.mdc` | — (install per-project) |

---

## Configuration

**Database path:** `~/.config/jira/jira.db`

Override with the `JIRA_DB` environment variable:

```sh
JIRA_DB=/custom/path/jira.db jira get PROJ-123
```

---

## Flags

| Flag | Applies to | Effect |
|---|---|---|
| `--instance NAME` | `get`, `current`, `mine`, `search`, `browse` | Force a specific configured instance |
| `--json` | `get`, `current`, `mine`, `search` | Print curated minified JSON for agents |
| `--refresh` | `get`, `current` | Bypass the cache and re-fetch from the API |
| `--no-comments` | `get`, `current` | Omit the comments section |
| `--download-attachments` | `get`, `current` | Download all attachments to the local downloads dir |

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Issue not found / HTTP error / parse error |
| 2 | Usage error, unknown instance, no instances configured, branch mismatch |

---

## Development

```sh
# Run all tests (unit + integration, including comment-policy gate)
docker compose run --rm dev cargo test

# Run only the comment-policy gate
docker compose run --rm dev cargo test --test comment_policy

# Lint
docker compose run --rm dev cargo clippy --all-targets -- -D warnings

# Format check
docker compose run --rm dev cargo fmt --check
```

---

## License

[MIT](LICENSE)
