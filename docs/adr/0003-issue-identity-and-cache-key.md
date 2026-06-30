---
type: ADR
title: "Issue identity = (instance_name, issue_key); cache keyed on it"
description: An issue is identified across jira-cli by the pair (instance_name, issue_key), replacing the AC base's (instance, project_id, task_id) triple, because the Jira issue key is globally unique within an instance. The SQLite cache and the agent_json ref are keyed on this pair.
status: Accepted
supersedes:
superseded_by:
tags: [data-model, cache, identity, jira]
timestamp: 2026-06-29T00:00:00Z
---

# 0003. Issue identity = (instance_name, issue_key); cache keyed on it

## Context

The AC base identifies a task by the triple `(instance_name, project_id, task_id)`
because ActiveCollab task ids are only unique within a project. Jira is different:
the **issue key** (`PROJ-123`) is globally unique within an instance and is the
identifier users actually type, paste, and see in branch names. Carrying AC's
triple into Jira would invent a project component the domain does not need and
would not match what `get` accepts.

## Decision

Identify an issue by the pair **`(instance_name, issue_key)`**.

1. **Cache key.** `CACHED_ISSUE` is keyed on `(instance_name, issue_key)`. A
   refresh re-fetches by key and overwrites the row. `project_key` is stored as a
   derived attribute (the prefix of the key) for grouping/display, not as part of
   the identity.
2. **`get` input.** `get` accepts the bare key (`PROJ-123`) or a browser URL
   (`https://<site>.atlassian.net/browse/PROJ-123`), from which the key is parsed.
3. **`agent_json` ref.** Every issue object carries `"ref":"PROJ-123"`, the exact
   form `get` accepts, so an agent can chain `mine`/`search` → `get`.
4. **Instance resolution.** When `--instance` is omitted and more than one is
   configured, resolution follows the AC convention (explicit flag, else a single
   configured instance, else an ambiguity error exit 2).

## Alternatives considered

- **Keep the `(instance, project_id, task_id)` triple.** Rejected: Jira issue ids
  are unique by key within an instance; the triple adds a project component the
  domain does not need and diverges from the `ref` users type.
- **Global key without instance.** Rejected: the same key can exist in two
  configured Cloud sites; the instance must be part of identity.

## Consequences

**Positive:**

- The cache key matches the user's mental model and the `get` input — no
  translation between identity and `ref`.
- Simpler schema than the AC base.

**Accepted trade-offs:**

- `project_key` becomes a derived display attribute; any future per-project
  grouping reads it rather than the identity.

## Related

- Constitution: [/constitution.md](/constitution.md)
- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- BDR: [/bdr/0001-get-issue-by-key.md](/bdr/0001-get-issue-by-key.md)
- BDR: [/bdr/0003-local-first-cache-read.md](/bdr/0003-local-first-cache-read.md)
