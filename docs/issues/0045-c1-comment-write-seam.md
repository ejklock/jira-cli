---
type: Issue
title: "C1 — comment write seam: add/update/delete_comment on JiraClient, ADF builder, author accountId, write-surface gate"
description: JiraClient trait + gouqi-backed impl gain the three comment write methods (POST/PUT/DELETE /rest/api/3/issue/{key}/comment[/{id}]) with curated CommentWriteResult and classify_error routing; pure plain_text_to_adf builder (paragraph + hardBreak); IssueComment gains author_account_id; tests/write_surface.rs source-scan gate enforces comments-only writes.
status: done
labels: [client, write, comments, parity]
blocked_by:
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## C1 — comment write seam

First Grupo C slice of [PRD 0003](/prd/0003-active-collab-parity.md)
(R-C1..R-C3 foundation), per [ADR 0015](/adr/0015-comment-write-enablement.md)
and [ADR 0022](/adr/0022-comment-write-seam.md). No user-observable behavior
yet (no BDR): this lands the seam every later comment slice consumes —
non-TTY `jira comment` (C2), TUI compose (C3), edit/delete (C4).

Scope: `src/client.rs` (trait + impl + comment mapper), `src/models.rs`
(`CommentWriteResult`, `author_account_id`), `tests/unit/client.rs`
(wiremock write contracts), `tests/unit/support.rs` (builder field),
`tests/write_surface.rs` (new gate). Mechanical `author_account_id: None`
additions to pre-existing fixture literals in `tests/unit/{models,agent_json,commands}.rs`.

Delivered with the ADR 0022 addendum's dot-segment workaround: gouqi 0.20.0
has no `put_versioned`/`delete_versioned`, so PUT/DELETE reach v3 through a
single `v3_write_endpoint` helper (`/../3` prefix, RFC 3986 normalization),
guarded by literal received-path wiremock asserts.

**Known follow-ups:** (a) contribute `put_versioned`/`delete_versioned`
upstream to gouqi and delete the workaround; (b) the pre-existing
`tests/comment_policy.rs` `find_line_comment` (cognitive 45) could share the
extracted `skip_string_literal` helper from `tests/write_surface.rs` via a
common test utility (review observation).
