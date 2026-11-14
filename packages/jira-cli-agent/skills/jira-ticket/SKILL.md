---
name: jira-ticket
description: Read Jira Cloud issue data — an issue, your assignments, or a JQL search — as machine-readable JSON from the `jira` CLI, non-interactively without the TUI. Use when an agent or script needs to fetch an issue by key or URL, list the logged-in user's open issues, read the issue for the current git branch, or run a JQL search, and wants structured JSON instead of the interactive terminal UI. Covers `jira get`, `jira current`, `jira mine`, and `jira search` with `--json` — the curated minified schemas, the round-trippable `ref`, and the cache / `--no-comments` / `--refresh` flags. Also covers posting a comment with `jira comment`. Also covers downloading every issue attachment to local disk with `--download-attachments`.
---

# jira-ticket (thin pointer)

The full, authoritative Jira Cloud `--json` read contract is served by the CLI itself.

Run:

    jira skill jira-ticket

and follow its output. It documents the curated minified JSON schemas for
`jira get`, `jira current`, `jira mine`, and `jira search` with `--json`, the
round-trippable `ref`, and the cache / `--no-comments` / `--refresh` flags.
