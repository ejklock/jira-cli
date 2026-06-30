---
type: ADR
title: "Curated, minified agent_json output contract (--json), inherited and re-shaped for Jira"
description: Inherit the AC agent_json discipline — a single curated, minified JSON object per read command, derived from the same helpers as the human renderer so JSON and text never drift — and re-shape the schemas for Jira issues, with --json forcing non-interactive output.
status: Accepted
supersedes:
superseded_by:
tags: [cli, json, agent, llm, contract]
timestamp: 2026-06-29T00:00:00Z
---

# 0004. Curated, minified agent_json output contract (--json)

## Context

The AC base exposes a curated, minified `--json` contract for a second consumer
class — LLM agents and scripts — derived from the same domain helpers as the human
renderer so the two never drift, with all shaping in a pure, unit-tested module.
`jira-cli` wants the same property: a stable, low-token, non-interactive contract.
Only the field set changes, because the domain is Jira issues, not AC tasks.

## Decision

Inherit the AC `agent_json` discipline and re-shape the schemas for Jira.

1. **Curated, not raw.** Each read command emits a small, stable object containing
   only the fields an agent needs, derived from the same helpers as the human
   renderer. No raw Jira payload dump.
2. **Minified.** One line, compact `serde_json::to_string`. Token-efficient.
3. **Uniform `--json`.** `get`, `current`, `mine`, and `search` all accept
   `--json`; for the list commands it forces non-interactive output.
4. **Pure module.** All shaping lives in `src/agent_json.rs` as pure functions over
   domain values, unit-tested without network. A field rename or drop fails a test.
5. **Round-trippable `ref`.** Every issue carries `"ref":"PROJ-123"`, the exact
   form `get` accepts ([ADR 0003](/adr/0003-issue-identity-and-cache-key.md)).

## Schemas

### `get` / `current` — one issue object

```json
{"ref":"PROJ-123","instance":"work","project_key":"PROJ","key":"PROJ-123",
 "summary":"...","status":"In Progress","status_category":"indeterminate",
 "issue_type":"Story","assignee":"Jane Doe","assignee_id":"5b10...","reporter":"John",
 "priority":"High","created":"2026-01-02T10:00:00.000+0000","updated":"2026-01-09T12:00:00.000+0000",
 "url":"https://acme.atlassian.net/browse/PROJ-123",
 "description":"plain text (ADF flattened)",
 "comments":[{"author":"John","author_id":"5b10...","created":"2026-01-03T14:22:00.000+0000","body":"plain text"}]}
```

- `status` is the literal Jira status name; `status_category` is the literal
  category key (`new` / `indeterminate` / `done`) — not a translated label.
- `assignee`/`reporter` are the resolved display names or `null`; ids are the
  `accountId` or `null`.
- `comments` is `[]` when `--no-comments` is set or there are none.
- The curated path is cache-aware and honours `--refresh` and `--no-comments`.

### `mine` / `search` — issue list

```json
{"count":2,"jql":"assignee = currentUser() AND statusCategory != Done",
 "issues":[{"ref":"PROJ-123","instance":"work","key":"PROJ-123","summary":"...",
   "status":"In Progress","issue_type":"Story","assignee":"Jane Doe"}]}
```

## Alternatives considered

- **Dump the raw Jira REST payload pretty-printed.** Rejected: verbose (token
  cost), unstable (coupled to the upstream ADF/shape), and the opposite of a
  stable agent contract.
- **A separate `--agent`/`--llm` flag.** Rejected: `--json` already means
  machine-readable; curating it is the smaller surface.

## Consequences

**Positive:**

- Agents get a stable, documented, low-token Jira contract locked by tests.
- `--json` on `mine`/`search` is non-interactive by definition.

**Accepted trade-offs:**

- Jira's rich description format (ADF) is flattened to plain text in the contract;
  consumers needing the structured body use the Jira API directly.

## Related

- Constitution: [/constitution.md](/constitution.md)
- PRD: [/prd/0001-jira-cloud-read-cli.md](/prd/0001-jira-cloud-read-cli.md)
- ADR: [/adr/0003-issue-identity-and-cache-key.md](/adr/0003-issue-identity-and-cache-key.md)
