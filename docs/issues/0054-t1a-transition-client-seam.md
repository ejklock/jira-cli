---
type: Issue
title: "T1a — transition client seam: list_transitions + transition_issue on the JiraClient trait (GET/POST the Jira transitions endpoints), with a Transition domain type"
description: Add the read + write transition seam to the client. The JiraClient trait grows list_transitions(key) -> Vec<Transition> (GET /rest/api/3/issue/{key}/transitions?expand=transitions.fields, parsed to a domain Transition { id, name, to_status, requires_fields }) and transition_issue(key, transition_id) -> () (POST /rest/api/3/issue/{key}/transitions with {transition:{id}}). Host-pinned via the existing GouqiJiraClient; 401 maps to the typed Unauthorized. Wiremock tests assert the exact GET path/expand + POST path/body and the required-fields parse; the client request-surface test is extended so the only non-GET endpoints are the comment endpoints + the transition POST (Constitution Amendment 2 falsifiable clause).
status: todo
labels: [client, transition, workflow, write, api, parity]
blocked_by:
tracker:
timestamp: 2026-07-16T00:00:00Z
---

## T1a — transition client seam

Implements [ADR 0027](/adr/0027-status-transition-write-enablement.md) §1, §2, §6
(the client half). Foundation slice for the status-transition feature — no TUI
yet.

Scope: `src/client.rs`, `src/models.rs`, `tests/unit/client.rs`.

- **Domain type** `Transition { id: String, name: String, to_status: String,
  requires_fields: bool }` on `src/models.rs` (serde-mapped from the Jira
  payload; `requires_fields` is true iff the `fields` map has any entry with
  `required: true`).
- **`list_transitions(key) -> ClientResult<Vec<Transition>>`** — GET
  `/rest/api/3/issue/{key}/transitions?expand=transitions.fields`, parsed to the
  domain type.
- **`transition_issue(key, transition_id) -> ClientResult<()>`** — POST
  `/rest/api/3/issue/{key}/transitions` with body `{"transition":{"id":<id>}}`.
- 401 → the typed `Unauthorized`; both requests reuse the host-pinned client.
- Wiremock tests: the exact GET path + `expand`, the required-fields parse
  (a mixed payload: one field-free, one field-requiring), the exact POST path +
  body, and the 401 mapping. Extend the client request-surface test so the only
  non-GET endpoints are the comment endpoints and the transition `POST`.
