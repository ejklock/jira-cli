---
type: Issue
title: "D2a — attachment download seam: download_attachment on JiraClient with same-origin guard"
description: Add async download_attachment(url) -> Bytes to the JiraClient trait and implement it on GouqiJiraClient with a reqwest Basic-auth GET, guarded by a same-origin check that rejects cross-origin URLs before any network call. Wiremock-tested. The shared seam D2b and D1 build on.
status: Proposed
timestamp: 2026-07-23T21:29:38Z
---

<!-- OKF frontmatter above carries the tracker metadata (`status`: open | in-progress |
     closed | superseded) that previously lived only in the directory index. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## D2a — attachment download seam

Implements [ADR 0029](/adr/0029-attachments-authenticated-download-seam-download-attachments-and-external-image-viewer.md) §1
and [BDR 0020](/bdr/0020-attachment-download-and-external-image-viewer-behaviors.md) S1–S3.
Foundation slice — the shared authenticated binary-fetch seam that D2b (download
flag) and D1 (image viewer) both build on. Mirrors the project's client-seam-first
slicing (T1a before T1b).

### Scope

`src/client.rs` (trait method + `GouqiJiraClient` impl + same-origin guard) and its
unit test module `tests/unit/client.rs`. No CLI flag, no TUI, no download
orchestration (those are D2b/D1).

### Acceptance

- `JiraClient` gains `async fn download_attachment(&self, url: &str) -> ClientResult<bytes::Bytes>`.
- `GouqiJiraClient::download_attachment` performs a `reqwest` GET with the instance's
  `email`/`token` Basic auth and returns the response bytes (S1) — wiremock-tested
  that the served body is returned verbatim.
- A `url` whose scheme+host+port differ from the instance `base_url` is rejected with
  a typed error and **zero** network calls (S2) — asserted via a wiremock server that
  records no requests.
- A same-origin content URL returning 401 surfaces the typed `Unauthorized` error (S3).
- `GouqiJiraClient` stores the credentials/base URL (or a preconfigured client)
  needed for the fetch; the existing gouqi-based read/write methods are unchanged.

### Plan

1. Store what the fetch needs on `GouqiJiraClient` (instance `base_url`, `email`,
   `token`, and a `reqwest::Client`) at construction — the one construction site.
2. Add the trait method; implement it: parse `url` + `base_url`, compare origin,
   short-circuit to a typed error on mismatch, else GET with `.basic_auth`.
3. Wiremock tests for S1/S2/S3.
