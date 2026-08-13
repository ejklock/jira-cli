---
type: Issue
title: "J0 — walking skeleton: fork scaffold + setup + get PROJ-123 end-to-end"
description: Fork the AC crate, swap the API layer, and ship setup (add/list/remove/test) + get <KEY|URL> against Jira Cloud, end-to-end and demoable.
status: done
tracker:
tags: [skeleton, setup, get, jira]
timestamp: 2026-06-29T00:00:00Z
---

# J0 — walking skeleton: fork scaffold + setup + get PROJ-123

## Objective link

North Star → [Constitution](/constitution.md) →
[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R1/R2/R7 →
[BDR 0002](/bdr/0002-setup-instance-management.md) (setup) +
[BDR 0001](/bdr/0001-get-issue-by-key.md) (get) → in force:
[ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md),
[ADR 0002](/adr/0002-jira-cloud-only-basic-auth.md),
[ADR 0003](/adr/0003-issue-identity-and-cache-key.md),
[ADR 0004](/adr/0004-agent-json-output-contract.md),
[ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md).

## Context manifest

Fork source: `active-collab-cli` (`../active-collab-cli`, same `www/klock-tecnologia`).

- **Copy + keep:** `Cargo.toml`/`Dockerfile`/`docker-compose.yml`/`Makefile`,
  `src/{main,cli,commands,controller,http,i18n}.rs`, `src/store/*`,
  `src/agent_json.rs`, `src/render.rs`, `tests/comment_policy.rs`, the
  wiremock+tempfile test harness. Rename the binary/crate to `jira`. Carry `tui/`
  over but do **not** wire `browse` into the CLI.
- **Rewrite (domain swap):**
  - `src/client.rs` — define the `JiraClient` trait (`get_issue(key)`,
    `search(jql, opts)`, `myself()`) and a **gouqi-backed impl**
    ([ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md)). Add the `gouqi`
    dependency (features `async`, reqwest `rustls-tls`); configure
    `SearchApiVersion::V3` and `Credentials::Basic(email, token)`. The wrapper is
    the single `gouqi::Jira` construction site, always from the resolved instance
    host (host-isolation invariant + unit test). Map gouqi `rep` types to our
    `models` at this boundary — gouqi types never leak into `agent_json`/`render`/cache.
  - `src/models.rs` — our domain shapes: `Issue` (key, summary, status,
    status_category, issue_type, assignee, priority, created, updated, description
    text, comments), `Myself` (accountId).
  - `src/store` cache — key `(instance_name, issue_key)` per
    [ADR 0003](/adr/0003-issue-identity-and-cache-key.md); add `account_id` column
    to the instance row.
  - `commands`: `setup` (add/list/remove/test), `get`.
  - `agent_json` — re-shape to the Jira `get` object
    ([ADR 0004](/adr/0004-agent-json-output-contract.md)).
  - ADF→plain-text flattener in `render`.
- **Build/run:** Docker only, bare commands from repo root (see CLAUDE.md).

## Vertical Demo

Run on the real stack (a real Jira Cloud site), exercising the unhappy path live.

- **Given** a clean checkout and a Jira Cloud API token,
  **When** I run
  `jira setup add --name work --url https://<site>.atlassian.net --email me@acme.com`
  and paste the token at the prompt,
  **Then** it prints `Instance 'work' saved.` and `Connectivity: OK`, and
  `jira setup list` shows the row with a resolved ACCOUNT_ID.
- **Given** the configured instance,
  **When** I run `jira get <a-real-KEY>` and then `jira get <a-real-KEY> --json`,
  **Then** the first prints the human render (summary/status/assignee/description)
  and the second prints one minified line whose `ref` equals the key.
- **Unhappy path (live):** **When** I run `jira get <KEY>` with a **bad token**
  configured (or a non-existent key), **Then** it prints the auth/not-found error
  and exits non-zero — no false success.

## Acceptance

| AC | Condition | Instrument |
|---|---|---|
| AC1 | Crate builds + clippy clean + fmt | `docker compose run --rm dev cargo build` / `clippy --all-targets -D warnings` / `fmt --check` (build) |
| AC2 | `setup add` resolves accountId + stores instance (Scn 1) | wiremock+temp-SQLite integration test |
| AC3 | `setup` validation/error/exit codes (BDR 0002 Scn 2,3,5) | integration tests |
| AC4 | `get` fetch+cache+render, key & URL (BDR 0001 Scn 1,2) | wiremock+temp-SQLite integration test |
| AC5 | `get --json` curated object, locked fields (BDR 0001 Scn 3) | `agent_json` unit + integration |
| AC6 | not-found / bad token → exit 1 (BDR 0001 Scn 4) | integration test |
| AC7 | Token host isolation — gouqi `Jira` is built only from the instance host; wrapper never requests another host (NFR-1) | unit test on the client wrapper (single construction site) |
| AC8 | No banner/commented-out code | `cargo test --test comment_policy` |
| AC9 (constraint) | No superfluous comments; only why-comments | inspection |
| AC10 (constraint) | Cyclomatic ≤ 10 (≤ 8 new fns), cognitive within gate | quality-gate complexity check |
| AC11 (constraint) | Tests assert observable behavior; surviving mutant on changed lines fails | quality-gate mutation check (report-only Rust → reviewer backstop) |
| AC12 (constraint) | Curated domain `models` — only fields used; not a 1:1 mirror of gouqi `rep` (design principle 1) | inspection (Reviewer) |
| AC13 (constraint) | Thin `commands` — orchestration lives in controller/client (design principle 2) | inspection (Reviewer) |
| AC14 (constraint) | Only `client` constructs `gouqi::Jira`; no gouqi type appears in agent_json/render/store (design principle 3) | inspection + grep for gouqi outside client |

## Out of scope (deferred)

- Offline cache read / `--refresh` semantics → [J1](/issues/0002-j1-local-first-cache.md).
- `current`, `mine`, `search` → J2–J4.
- The `browse` TUI; writes; Jira Server/DC; secret encryption.

## blocked_by

— (root slice)
