---
type: Issue
title: D2b — jira get --download-attachments writes every attachment to the config downloads dir
description: Add a --download-attachments flag (and optional --download-dir) to jira get/current that downloads every attachment via the D2a seam and writes it to ~/.config/jira/downloads/<ISSUE-KEY>/, deduping filenames, with human 'saved <path>' output and a curated --json listing.
status: done
timestamp: 2026-07-23T21:29:38Z
---

<!-- OKF frontmatter above carries the tracker metadata (`status`: open | in-progress |
     closed | superseded) that previously lived only in the directory index. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## D2b — `jira get --download-attachments`

Implements [ADR 0029](/adr/0029-attachments-authenticated-download-seam-download-attachments-and-external-image-viewer.md) §2
and [BDR 0020](/bdr/0020-attachment-download-and-external-image-viewer-behaviors.md) S4–S7.
Builds on the D2a seam ([issue 0060](/issues/0060-d2a-attachment-download-seam-download-attachment-on-jiraclient-with-same-origin-guard.md)).

### Scope

`src/config.rs` (`resolve_download_dir`), `src/download.rs` (new — pure
`dedupe_filename` + the download orchestration), `src/cli.rs` (`--download-attachments`
+ `--download-dir` on `DisplayArgs`), `src/main.rs` (wiring after issue fetch), and
tests (`tests/unit/download.rs`, an integration test). KEPT: the existing `get`
rendering path is unchanged when the flag is absent. Out of scope: any TUI change,
per-file selection (v1 downloads all), a human attachments section in `render.rs`.

### Acceptance

- `resolve_download_dir(key)` returns `~/.config/jira/downloads/<KEY>/` reusing the
  same root helper as `resolve_db_path` (unit-tested via env/root override).
- `dedupe_filename(taken, name)` returns a name not in `taken`, suffixing ` (2)`,
  ` (3)` before the extension; property-tested that the result is never in `taken` (S6).
- `jira get <KEY> --download-attachments --download-dir <tmp>` writes every attachment
  to `<tmp>` and prints `saved <path> (<bytes>)` per file (S4) — integration test.
- With `--json`, stdout is one curated object listing each `{ filename, path, bytes }`
  (S5) — integration test.
- An issue with no attachments prints a clear message and exits 0, writing nothing (S7).
- A failed download is reported as an error, never a false success.

### Plan

1. `config.rs`: extract a `~/.config/jira/` root helper; add `resolve_download_dir`.
2. `download.rs`: pure `dedupe_filename`; an orchestration fn that takes the client +
   issue + target dir, loops attachments (seam → dedupe → write), returns the saved
   results for the caller to render.
3. `cli.rs`: `download_attachments: bool`, `download_dir: Option<PathBuf>` on `DisplayArgs`.
4. `main.rs`: after fetch, when the flag is set, run the orchestration and render
   (human lines or `--json` object) instead of / alongside the normal output.
5. Unit tests for the pure helpers; an integration test driving the flag against a
   mocked client into a temp dir.
