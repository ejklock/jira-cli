---
type: ADR
title: "Comment write seam — gouqi write verbs behind the JiraClient trait, plain-text→ADF builder, comment author identity"
description: Implement ADR 0015's write surface as three trait methods (add_comment / update_comment / delete_comment) on JiraClient, issued through the wrapped host-pinned gouqi::Jira write-verb helpers (never a second HTTP stack), with a pure plain-text→minimal-ADF builder (paragraph + hardBreak), a curated CommentWriteResult, author accountId retained on IssueComment for the own-comment predicate, and a source-scan write-surface gate test as the constitution's falsifiable clause.
status: Accepted
supersedes:
superseded_by:
tags: [client, write, comments, adf, security, api]
timestamp: 2026-07-07T00:00:00Z
---

# 0022. Comment write seam

## Context

[ADR 0015](/adr/0015-comment-write-enablement.md) fixed the write surface
(Jira Cloud comment endpoints only), body format (minimal ADF), the
own-comment permission model, and server-truth refresh. This ADR fixes the
seam mechanics inside `src/client.rs` / `src/models.rs`.

Today the `JiraClient` trait exposes five read methods; the gouqi-backed
impl (`GouqiJiraClient`) is the **single** `gouqi::Jira` construction site,
host-pinned to the instance `base_url` (the token-isolation invariant,
[ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md)). Reads go through
gouqi's versioned GET helper. `IssueComment` carries `id` and a display
`author`, but not the author's `accountId` — the field the own-comment
affordance predicate (ADR 0015 §3) needs. The ADF read mapper
(`render::adf_to_plain_text` / `adf_to_rich`) has no inverse.

## Decision

1. **Three trait methods, curated returns.** `JiraClient` grows

   ```rust
   async fn add_comment(&self, key: &str, body_text: &str) -> Result<CommentWriteResult, ClientError>;
   async fn update_comment(&self, key: &str, comment_id: &str, body_text: &str) -> Result<CommentWriteResult, ClientError>;
   async fn delete_comment(&self, key: &str, comment_id: &str) -> Result<(), ClientError>;
   ```

   `CommentWriteResult { id: String }` is a curated model (guardrail 1:
   never a raw `serde_json::Value` across the trait). Endpoints:
   `POST /rest/api/3/issue/{key}/comment`, `PUT .../comment/{id}`,
   `DELETE .../comment/{id}` (204, no body).
2. **Writes ride the wrapped gouqi instance.** The impl calls gouqi's
   write-verb helpers (versioned POST/PUT/DELETE, mirroring the existing
   versioned-GET call shape) on the already-constructed, host-pinned
   `gouqi::Jira`. **No second HTTP client, no new construction site** — the
   token-isolation invariant is inherited, not re-implemented. Non-2xx and
   transport errors route through the existing `classify_error`
   (401 → `ClientError::Unauthorized{instance}`, the R-E2 re-auth contract;
   403 and others surface as `ClientError::Other` with the status context —
   never a false `Ok`).
3. **Pure ADF builder.** `plain_text_to_adf(text) -> serde_json::Value`
   (pure, unit-tested) emits the minimal document ADR 0015 §2 fixed: one
   `doc` → one `paragraph` whose content alternates `text` nodes with
   `hardBreak` nodes for `\n`. It is the inverse of the read mapper for
   exactly the subset the compose surface produces.
4. **Author identity on the read model.** `IssueComment` gains
   `author_account_id: Option<String>` (`#[serde(default)]`, additive),
   mapped from `author.accountId` in the client's comment extraction. The
   own-comment predicate (C3/C4 slices) becomes
   `comment.author_account_id == instance.account_id` — data lands now so
   the TUI slices are model-only diffs.
5. **Falsifiable write-surface gate.** A source-scan integration test
   (`tests/write_surface.rs`, same family as `tests/comment_policy.rs`)
   asserts every gouqi write-verb call site in `src/` lives in
   `src/client.rs` inside the three comment methods / targets a comment
   endpoint path. This is the unit-test enforcement ADR 0015 §1 promised
   for Constitution Amendment 1.

### Addendum — PUT/DELETE versioning workaround (2026-07-07)

Implementation surfaced that gouqi 0.20.0 (the latest published release) has
**no** `put_versioned`/`delete_versioned` — only GET/POST have versioned
helpers; the unversioned `put`/`delete` build `rest/{api}/latest{endpoint}`,
which aliases the non-ADF v2 surface (the same reason `get_issue` already
bypasses `issues().get()`). Decision: keep the single wrapped instance and
reach v3 through **RFC 3986 dot-segment normalization** — a single private
helper prefixes the endpoint with `/../3` so the `url` crate's parse-time
path normalization collapses `latest/../3` into `3`. The dependence on the
third-party URL builder is made falsifiable: the wiremock write tests assert
the **received** request path is literally `/rest/api/3/...`, so any gouqi
or `url`-crate change breaks the build, never production. Follow-up:
contribute `put_versioned`/`delete_versioned` upstream and delete the
workaround. Rejected here: forking gouqi (maintenance/supply-chain cost for
two helpers) and accepting the `latest` v2 alias (unverifiable ADF-body
compatibility — silent production risk).

## Alternatives considered

- **A raw reqwest fallback for writes.** Rejected — a second network stack
  breaks the single-construction-site invariant that makes host isolation
  auditable.
- **Returning the full created Comment.** Rejected — callers need only the
  id (the thread is re-fetched server-truth after every write, ADR 0015 §4);
  a full comment return invites optimistic patching.
- **Paragraph-per-line ADF.** Rejected — `hardBreak` inside one paragraph
  matches what Jira's own compose produces for plain newlines and keeps the
  builder trivially invertible by the existing flattener.

## Consequences

**Positive:** the write surface is three narrow methods behind the existing
trait seam; every later slice (non-TTY command, TUI compose/edit/delete) is
a consumer, not a new capability. The gate test makes "comments only" a
failing build, not a promise. **Trade-offs:** the trait is no longer
read-only (accepted by Amendment 1); mocked-server tests grow write
fixtures; `author_account_id` adds one mechanical field to existing test
fixtures.

## Related

- ADR: [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md) (the enablement this implements)
- ADR: [/adr/0005-jira-client-on-gouqi-behind-trait.md](/adr/0005-jira-client-on-gouqi-behind-trait.md) (the trait seam + host pinning)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) (R-C1..R-C3)
- Fork base: `active-collab-cli` ADR 0033 (authenticated write seam)
- Issue: [/issues/0045-c1-comment-write-seam.md](/issues/0045-c1-comment-write-seam.md)
