---
type: ADR
title: "Attachments: authenticated download seam, --download-attachments, and external image viewer"
description: Add a same-origin authenticated download_attachment seam to JiraClient, a jira get --download-attachments flag that writes every attachment to ~/.config/jira/downloads/<ISSUE-KEY>/, and a browse-TUI image viewer that opens attachments in the OS viewer — porting ActiveCollab ADR 0065/0066 to jira-cli.
status: Accepted
timestamp: 2026-07-23T21:27:36Z
---

# 0029. Attachments: authenticated download seam, `--download-attachments`, and external image viewer

## Context

Issues already carry curated attachment metadata — `Attachment { filename, url,
mime_type, size }` ([ADR 0020](/adr/0020-issue-attachments-detail-panel.md)) —
surfaced in the `--json` contract (`shape_attachment`) and rendered as selectable
links in the browse TUI (`attachments_panel`). But the tool can only show that an
attachment *exists*: there is **no way to fetch its bytes**. The `JiraClient` trait
exposes reads and comment/transition writes; none returns binary content.

The fork base (`active-collab-cli`) closed the same gap with two features — an
in-app image viewer (ActiveCollab ADR 0065) and a bulk download flag (ActiveCollab
ADR 0066). This ADR ports both to jira-cli as Épico D of the parity program,
adapted to Jira Cloud's REST/auth model.

Three forces shape the design:

- **`Attachment.url` is the Jira `content` endpoint** —
  `https://<host>/rest/api/3/attachment/content/{id}` — and returns the raw bytes
  only to an **authenticated** request. gouqi 0.20 has no binary-fetch helper, but
  `reqwest` (rustls) is already a dependency.
- **The URL is server-supplied JSON.** Blindly sending the instance's Basic-auth
  header to whatever host that field names would leak credentials if a response
  were ever tampered with. [ADR 0002](/adr/0002-jira-cloud-only-basic-auth.md) pins
  the tool to one instance host; the download path must honour that.
- **Terminal image protocols (kitty/iterm2/sixel) are non-portable** and pull in a
  heavy `image` decode stack. The app already resolves a per-user config dir
  (`~/.config/jira/`) and can shell out to the OS viewer.

## Decision

We will add **one authenticated, same-origin download seam** to `JiraClient` and
build both user features on top of it: a bulk `--download-attachments` flag and a
TUI image viewer that opens the file in the OS's own viewer.

1. **Download seam (slice D2a).** Extend the `JiraClient` trait with
   `async fn download_attachment(&self, url: &str) -> ClientResult<bytes::Bytes>`.
   `GouqiJiraClient` implements it with a `reqwest::Client` and the instance's
   `email`/`token` as Basic auth. **Same-origin guard:** before the request the
   impl parses `url` and the instance `base_url` and rejects — with a typed error,
   no network call — any `url` whose scheme+host+port differ from the instance, so
   credentials never leave the instance host. This is the only construction site
   for a binary fetch.

2. **`jira get --download-attachments` (slice D2b).** A boolean flag on the shared
   `DisplayArgs` (so `get` and `current` both accept it). After the issue is
   fetched, the tool downloads **every** attachment via the seam and writes each to
   the target dir; an optional `--download-dir <DIR>` overrides the default.
   - **Target dir:** `~/.config/jira/downloads/<ISSUE-KEY>/`, resolved by a new
     `resolve_download_dir(issue_key)` in `config.rs` reusing the same
     `~/.config/jira/` root as `resolve_db_path`. Created if absent.
   - **Filename collisions:** Jira permits duplicate filenames on one issue. A pure
     `dedupe_filename(taken, filename)` helper suffixes ` (2)`, ` (3)`, … before the
     extension so no download silently overwrites another.
   - **Output:** human mode prints one `saved <path> (<bytes>)` line per file;
     `--json` mode emits a curated object listing `{ filename, path, bytes }` per
     saved attachment (never a false success — a failed download is reported).
   - An issue with **no** attachments is a clean no-op with a clear message, exit 0.

3. **External image viewer (slice D1).** In the browse TUI, a focused **image**
   attachment (mime type `image/*`) triggers a new `Cmd::OpenAttachment`: the shell
   downloads the bytes via the seam, writes them to a temp file (`tempfile`), and
   shells out to the platform opener (`open` macOS, `xdg-open` Linux, `start`
   Windows). A focused **non-image** attachment keeps the existing
   open-URL-in-browser behavior. The pure `update` layer only emits the `Cmd`; all
   I/O stays in `dispatch_cmd`, preserving the model's purity discipline.

**Alternatives rejected:** inline terminal-graphics rendering (non-portable, heavy
`image` dep); trusting `Attachment.url`'s host as-is (credential-leak vector);
downloading through gouqi (no binary GET in 0.20); defaulting the download target to
the CWD (the operator chose the per-user config dir to keep the CWD clean).

## Consequences

**Easier / gained:**
- Attachments become fully usable — fetchable in bulk from scripts/agents and
  viewable from the TUI — closing the last read-parity gap with the fork base.
- One audited seam owns every binary fetch, with a same-origin guard that keeps
  Basic-auth credentials on the instance host.
- The viewer adds no terminal-protocol or image-decode dependency; it works in any
  terminal.

**Harder / accepted trade-offs:**
- The viewer depends on an OS opener being present (documented); headless
  environments fall back to the download flow.
- `--download-attachments` fetches **all** attachments (no per-file selection in
  v1); the TUI viewer covers the single-file case.
- The same-origin guard rejects the unusual case of a tenant serving attachment
  content from a different host; revisit only if a real tenant needs it.

**Follow-ups:**
- Slices D2a → D2b → D1 (issues
  [0060](/issues/0060-d2a-attachment-download-seam-download-attachment-on-jiraclient-with-same-origin-guard.md),
  [0061](/issues/0061-d2b-jira-get-download-attachments-writes-every-attachment-to-the-config-downloads-dir.md),
  [0062](/issues/0062-d1-browse-tui-opens-an-image-attachment-in-the-os-viewer.md)).

## Verification

**Implementation impact:** `src/client.rs` (trait + impl + same-origin guard),
`src/config.rs` (`resolve_download_dir`), `src/download.rs` (new — pure
`dedupe_filename` + orchestration), `src/cli.rs` (flags), `src/main.rs` (wiring),
`src/tui/model.rs` + `src/tui/shell.rs` (viewer `Cmd`/`Msg` + dispatch).

**Verification criteria:**
- `download_attachment` returns bytes for a same-origin URL and a typed error
  (no network call) for a cross-origin URL — wiremock + unit test.
- `dedupe_filename` never returns a name already in `taken` — unit test.
- `jira get --download-attachments --download-dir <tmp>` writes every attachment to
  `<tmp>` and prints (or, with `--json`, lists) each saved path — integration test.
- A focused non-image attachment still opens its URL in the browser (unchanged) —
  TUI unit test on the key→`Msg`→`Cmd` mapping.

# References

[1] ActiveCollab ADR 0065 (image viewer), ADR 0066 (attachment download) — upstream parity source.
