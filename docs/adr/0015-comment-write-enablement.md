---
type: ADR
title: "Comment write enablement — POST/PUT/DELETE on the Jira comment endpoints only"
description: Enable creating, editing, and deleting the user's own comments (TUI compose modal + non-TTY command), as the single write surface allowed by Constitution Amendment 1. Server-truth refresh after every write; token host isolation extends to write requests; everything else stays read-only.
status: Accepted
supersedes:
superseded_by:
tags: [write, comments, api, security, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0015. Comment write enablement

## Context

The constitution declared v1 read-only, with write "a deliberate later slice,
behind its own ADR". The parity program ([PRD 0003](/prd/0003-active-collab-parity.md)
R-C1..R-C3) brings the fork base's comment write feature set (its C1–C3, M1/M2,
L1/L2, non-TTY `comment`) to jira-cli. Constitution **Amendment 1** (2026-07-06)
narrows the write exclusion to exactly comment writes. This ADR fixes HOW.

## Decision

1. **Write surface = the Jira Cloud comment endpoints, nothing else.**
   `POST /rest/api/3/issue/{key}/comment`, `PUT .../comment/{id}`,
   `DELETE .../comment/{id}`. The client trait (ADR 0005) grows
   `add_comment` / `update_comment` / `delete_comment`; no other non-GET
   method exists. Enforced by a unit test asserting the client's request
   surface (the constitution's falsifiable clause).
2. **Body format: ADF.** Comments are written as a minimal ADF document
   (paragraph/text with hardBreak for newlines) — the inverse of the read
   mapper; plain text in, ADF out. No rich compose in v1 of write.
3. **Own-comment permission model.** Edit/delete affordances render only on
   comments whose author `accountId` equals the instance's resolved
   `account_id`; the server remains the authority (a 403 surfaces on the
   status line). Comment ids and author accountIds are retained on the
   domain `Comment` model (additive, serde default).
4. **Server-truth refresh.** After any successful write, re-fetch the issue
   (busting cache) and re-render from the server payload — never patch the
   local model optimistically. Cache stays a read concern.
5. **Failure semantics.** Non-2xx → the write is not retried, the compose
   content is preserved (TUI) for retry/abandon, the error goes to the thin
   status line (TUI) or stderr + non-zero exit (CLI). 401 follows the R-E2
   re-auth contract.
6. **Token host isolation extends to writes.** Write requests reuse the
   host-pinned client; the token isolation test suite gains write-path cases.

## Alternatives considered

- **Full write parity later (issues, transitions, worklog).** Out — the
  amendment deliberately narrows to comments; each further write would need
  its own amendment + ADR.
- **Optimistic local update.** Rejected — the fork base's server-truth
  refresh proved simpler and immune to divergence; comments are low-latency.
- **Wiki-markup body (`representation=wiki`).** Rejected — REST v3 is
  ADF-native; wiki markup is a legacy detour.

## Consequences

**Positive:** the daily "reply from the terminal" flow lands; write surface is
minimal, testable, and constitution-fenced. **Trade-offs:** the client trait is
no longer read-only (NFR-B4 in PRD 0002 is narrowed accordingly for the
comment Cmds); mocked-server tests grow write fixtures; a compose modal enters
the TEA model (typed overlay state, port of the AC M1/DetailOverlay end state).

## Related

- Constitution: [/constitution.md](/constitution.md) (Amendment 1)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md)
- ADR: [/adr/0005-jira-client-on-gouqi-behind-trait.md](/adr/0005-jira-client-on-gouqi-behind-trait.md)
- Fork base: `active-collab-cli` issues 0032–0034, 0037–0041, PRD 0002 (writes)
