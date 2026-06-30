---
type: ADR
title: "Fork active-collab-cli and swap the API layer to Jira Cloud"
description: Build jira-cli by forking the active-collab-cli Rust codebase and replacing only the client/models/auth/commands domain layer with Jira Cloud, inheriting the architecture, cache, agent_json contract, i18n, Docker build, and docs trail.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, fork, reuse, rust]
timestamp: 2026-06-29T00:00:00Z
---

# 0001. Fork active-collab-cli and swap the API layer to Jira Cloud

## Context

`jira-cli` solves the same shape of problem as `active-collab-cli` (AC): a
local-first, single-binary terminal tool that reads and browses work items across
multiple instances, with a curated `agent_json` contract for scripts. AC is a
mature Rust project (clap + ratatui + reqwest + rusqlite + tokio) with a proven
architecture: a pure, testable core; a `store` that owns SQLite; a host-isolated
HTTP boundary; an `agent_json` schema locked by tests; en/pt-BR i18n; a Docker
build; and a living-docs trail (constitution, ADRs, BDRs).

Force: **time-to-value and risk.** Reusing AC's proven structure means the TUI,
cache, i18n, output contract, and build are already designed and tested. The only
genuinely new surface for Jira is the API/auth/domain layer.

The two domains differ in ways that must be re-modeled, not copied blindly: Jira
identifies issues by a globally-unique key (`PROJ-123`, not a `project_id/task_id`
pair), authenticates with Basic auth (email + API token, not a token-exchange),
lists via JQL (not per-project task lists), and has its own issue/status/issue-type
vocabulary.

## Decision

Build `jira-cli` as a **fork of the AC Rust codebase**, replacing the domain layer.

1. **Inherit wholesale:** the module shape (`cli`, `commands`, `controller`,
   `http`, `store`, `render`, `agent_json`, `i18n`, `tui`), the SQLite store
   pattern, the host-isolation gate, the `agent_json` contract
   ([ADR 0004](/adr/0004-agent-json-output-contract.md)), the i18n catalog
   structure, the Docker build, and the living-docs trail.
2. **Rewrite the domain layer:** `client` (Jira Cloud REST v3), `models` (serde
   shapes for Jira issue/search payloads), the auth path (Basic auth — see
   [ADR 0002](/adr/0002-jira-cloud-only-basic-auth.md)), the store cache keys
   ([ADR 0003](/adr/0003-issue-identity-and-cache-key.md)), and the command
   handlers (`get`, `current`, `mine`, `search`, `setup`).
3. **Bring the TUI across dormant.** The `tui/` module is carried over but the
   interactive `browse` command is **not** wired into v1; it is re-enabled as its
   own vertical slices in Phase 2. v1 is CLI-first.
4. **Rename the binary** to `jira` and the crate accordingly; preserve the AC
   conventions (no `cd`, Docker build, comment-policy test).

## Alternatives considered

- **Reuse patterns, fresh codebase.** Rejected for v1: rewriting the proven TUI,
  store, i18n, and render layers from scratch buys a marginally cleaner
  Jira-shaped model at the cost of re-deriving and re-testing everything AC
  already proved. The fork keeps that value; the domain swap is where the real
  modeling happens anyway.
- **Process scaffold only (free stack choice, e.g. the Python `jira-ticket` skill).**
  Rejected: discards AC's biggest reusable assets (single-binary distribution,
  the TUI, the locked output contract) for stack freedom we do not need.

## Consequences

**Positive:**

- v1 reaches a demoable `get` quickly: auth + client + render + cache is the only
  net-new code on the critical path.
- The output contract, cache discipline, and build come pre-tested.

**Accepted trade-offs:**

- The fork carries AC assumptions that must be deliberately re-modeled for Jira
  (issue identity, auth, listing). ADRs 0002–0003 record those re-models.
- Carrying the dormant `tui/` adds code that v1 does not exercise; it is kept
  compiling and is re-activated in Phase 2 rather than deleted and rebuilt.

## Related

- Constitution: [/constitution.md](/constitution.md)
- PRD: [/prd/0001-jira-cloud-read-cli.md](/prd/0001-jira-cloud-read-cli.md)
- ADR: [/adr/0002-jira-cloud-only-basic-auth.md](/adr/0002-jira-cloud-only-basic-auth.md)
- ADR: [/adr/0003-issue-identity-and-cache-key.md](/adr/0003-issue-identity-and-cache-key.md)
