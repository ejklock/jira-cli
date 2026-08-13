---
name: jira-ticket
description: Read Jira Cloud issue data — an issue, your assignments, or a JQL search — as machine-readable JSON from the `jira` CLI, non-interactively without the TUI. Use when an agent or script needs to fetch an issue by key or URL, list the logged-in user's open issues, read the issue for the current git branch, or run a JQL search, and wants structured JSON instead of the interactive terminal UI. Covers `jira get`, `jira current`, `jira mine`, and `jira search` with `--json` — the curated minified schemas, the round-trippable `ref`, and the cache / `--no-comments` / `--refresh` flags. Also covers posting a comment with `jira comment`. Also covers downloading every issue attachment to local disk with `--download-attachments`.
---

# jira --json — agent read contract

The `jira` binary exposes a single curated, **minified** JSON contract for
agent/LLM and script consumers across its read commands. `--json` is
**non-interactive**: on `mine`/`search` (and the interactive-by-default read
commands) it prints the JSON and exits without launching the browse TUI, even on
a terminal. The fields are derived from the same renderers as the human output,
so JSON and text never drift.

Authoritative decision: [ADR 0004](../../../docs/adr/0004-agent-json-output-contract.md).
Observable behavior + test matrix: [BDR 0001](../../../docs/bdr/0001-get-issue-by-key.md),
[BDR 0005](../../../docs/bdr/0005-mine-and-search-jql.md).

## Commands

| Command | Emits | TUI? |
|---|---|---|
| `jira get <ref> --json` | one issue object | n/a |
| `jira current --json` | one issue object (issue on the current git branch) | n/a |
| `jira mine --json` | `{count, jql, issues[]}` (issues assigned to you) | never launches |
| `jira search <jql> --json` | `{count, jql, issues[]}` (issues matching the JQL) | never launches |

All output is a **single line** (`serde_json::to_string`, compact). Pipe to a
formatter (`| jq .`) only for human reading — agents should parse the line
directly.

## The `ref` — chain mine/search → get

Every issue in every schema carries `"ref": "PROJ-123"`, the exact form `jira
get` accepts (the bare issue key). Discover with `mine`/`search`, then fetch
detail:

```bash
jira mine --json                       # → {"count":1,"jql":"...","issues":[{"ref":"PROJ-123",...}]}
jira get PROJ-123 --json               # → the full issue object for that ref
```

## Schemas

### `get` / `current` — one issue object

```json
{"ref":"PROJ-123","instance":"work","project_key":"PROJ","key":"PROJ-123",
 "summary":"...","status":"In Progress","status_category":"indeterminate",
 "issue_type":"Story","assignee":"Jane Doe","assignee_id":"5b10...","reporter":"John",
 "reporter_id":"5b10...","priority":"High",
 "created":"2026-01-02T10:00:00.000+0000","updated":"2026-01-09T12:00:00.000+0000",
 "duedate":"2026-01-15",
 "url":"https://acme.atlassian.net/browse/PROJ-123",
 "description":"plain text (ADF flattened)",
 "comments":[{"author":"John","author_id":null,"created":"2026-01-03T14:22:00.000+0000","body":"plain text"}],
 "attachments":[{"filename":"spec.pdf","url":"https://...","mime_type":"application/pdf","size":12345}]}
```

- `status` is the literal Jira status name; `status_category` is the literal
  category **key** (`new` / `indeterminate` / `done`), never a translated label.
- `assignee` / `reporter` are the resolved display names or `null`; `assignee_id`
  / `reporter_id` are the `accountId` or `null`.
- `duedate` is `null` when absent.
- `description` and comment `body` are plain text (ADF flattened via the same
  helper as the human renderer). Consumers needing the structured body use the
  Jira REST API directly.
- `comments` is `[]` when there are none or `--no-comments` is passed.
- `attachments` is `[]` when there are none.

### `mine` / `search` — issue list

```json
{"count":2,"jql":"assignee = currentUser() AND statusCategory != Done",
 "issues":[{"key":"PROJ-123","type":"Story","status":"In Progress",
   "assignee":"Jane Doe","summary":"..."}]}
```

- `jql` echoes the exact JQL the list resolved to.
- Each list row is a compact projection; fetch a `ref`/`key` with `jira get` for
  the full object.

## Flags

| Flag | Effect | Applies to |
|---|---|---|
| `--json` | curated minified JSON; non-interactive | `get`, `current`, `mine`, `search` |
| `--no-comments` | omit the `comments` array (emits `[]`) | `get`, `current` |
| `--refresh` | ignore the cache and re-fetch | `get`, `current` |
| `--instance <name>` | limit to one configured instance | `get`, `current`, `mine`, `search` |
| `--download-attachments` | download every attachment on the issue to local disk instead of rendering it | `get`, `current` |
| `--download-dir <DIR>` | override the default download directory for `--download-attachments` | `get`, `current` |

The `get`/`current` JSON path is **cache-aware** and honours `--refresh` and
`--no-comments`. The human (non-`--json`) output of every command is unchanged.

## Downloading attachments for local analysis (`--download-attachments`)

`--download-attachments` fetches every attachment on the issue over the CLI's
authenticated seam and writes each to disk. An agent can then `Read` the file
directly instead of following a remote URL. The flag downloads only — it never
also renders the issue:

```bash
jira get PROJ-123 --download-attachments --json
```

Without `--download-dir`, files land in a stable, predictable per-issue path:
`~/.config/jira/downloads/<ISSUE-KEY>/`. A duplicate filename on the same
issue gets a disambiguating suffix, such as `report (2).pdf`, before it
overwrites another file.

A single failed download aborts the whole request and exits non-zero. The
command never reports partial success for a batch of attachments.

With `--json`, the command prints a curated result instead of the issue
schema:

```json
{"issue_key":"PROJ-123","saved":[{"filename":"spec.pdf","path":"/home/user/.config/jira/downloads/PROJ-123/spec.pdf","bytes":45210}]}
```

Human mode prints one `saved <path> (<bytes>)` line per saved file. An issue
with no attachments prints a no-op message and exits 0.

## Writing a comment — `jira comment`

`jira comment [ISSUE_REF] -m "<body>"` posts a comment to an issue as the
logged-in user. Omit `ISSUE_REF` to resolve the issue from the current git
branch; omit `-m/--message` to read the body from stdin. `--json` prints a
curated minified write result; `--instance <name>` forces a configured instance.

```bash
jira comment PROJ-123 -m "Deployed the fix to staging."   # explicit ref
jira comment -m "Deployed the fix to staging."            # ref from git branch
jira comment PROJ-123 < body.txt                          # body from stdin
```

The body is plain text; the CLI wraps it into a minimal ADF document before
posting (Jira Cloud stores comment bodies as ADF). The `--json` write result is:

```json
{"ok":true,"comment_id":"10042","issue_key":"PROJ-123"}
```

On failure it is `{"ok":false,"error":"..."}` — never a false success.

## Notes for agents

- Parse the single line as one JSON object; do not assume pretty-printing.
- The schema is locked by unit tests in `tests/unit/agent_json.rs` — a field
  rename or drop fails a test, so these shapes are stable.
- Need the raw upstream Jira REST payload (structured ADF, all fields)? Use the
  Jira Cloud REST API directly; this CLI's `--json` is the **curated contract**,
  not a passthrough.

## Sandboxed environments

The CLI stores its SQLite database at `~/.config/jira/jira.db` by default. In
a sandboxed agent environment, that config directory can be read-only.

- Set `JIRA_DB` to point the CLI at a writable SQLite file instead of the
  default path.
- When the default config directory is read-only, copy the existing database
  to a writable path first, then point `JIRA_DB` at the copy:

```bash
cp ~/.config/jira/jira.db /tmp/jira.db && JIRA_DB=/tmp/jira.db jira get PROJ-123 --json
```

- A fresh, empty database has no configured instances. Every command fails
  with `no instances configured` until you run `jira setup add` against it.
