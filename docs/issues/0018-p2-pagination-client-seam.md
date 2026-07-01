---
type: Issue
title: "P2 — pagination client seam: expose SearchResult.next_page_token + JiraClient::search_page"
description: Expose gouqi's V3 nextPageToken through the domain SearchResult and add a JiraClient::search_page(jql, max_results, page_token) method that fetches a subsequent page via SearchOptionsBuilder::next_page_token. Additive, read-only, behind the existing single client seam; the CLI mine/search first-page path is unchanged.
status: done
tracker:
tags: [client, pagination, phase2, read]
timestamp: 2026-06-30T00:00:00Z
---

# P2 — pagination client seam: expose next_page_token + search_page

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) open question "in-TUI paging", resolved by
[ADR 0009](/adr/0009-tui-list-pagination.md), extending the single client seam
[ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md). Feasibility confirmed by the
spike (gouqi 0.20 `rep.rs:1007` `next_page_token`, `builder.rs:175` `next_page_token(&str)`).
Verified by [BDR 0006](/bdr/0006-browse-tui-interactions.md) (search_page fetch row).

## Context manifest

- **Read first:** `src/models.rs` — `SearchResult` (L59–65): `issues`, `total`,
  `is_last_page: bool` (`derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)`).
- `src/client.rs` — the `JiraClient` trait (L11–15: `get_issue`/`search`/`myself`);
  `GouqiJiraClient::search` (L50–77) builds `SearchOptions::builder().max_results(capped)`,
  calls `self.jira.search().list(jql, &opts)`, and maps `raw.issues`/`raw.total`/
  `raw.is_last_page.unwrap_or(true)` — it currently **drops** `raw.next_page_token`.
  `GouqiJiraClient` is the ONLY impl of the trait (no mock; tests use wiremock).
- `tests/unit/client.rs` — `search_returns_issue_rows_with_mapped_fields` (L302) is the
  wiremock pattern to mirror for `search_page` (the mock payload already carries
  `nextPageToken`); `tests/unit/models.rs` (L73–99) constructs a `SearchResult` literal and
  round-trips serde — it must gain the new field.
- Callers of `search` that must keep compiling unchanged: `src/tui/shell.rs` (L55, L233) and
  `src/commands.rs` (L580 mine, L643 search_core) — all read `.issues`, so an additive field
  does not touch them.

## Approach (decided — see ADR 0009)

- Add `pub next_page_token: Option<String>` to `SearchResult` (additive; `Option` serde is
  absent-by-default, so the JSON contract stays compatible and `agent_json`/CLI ignore it).
- `GouqiJiraClient::search`: map `raw.next_page_token` through into the returned
  `SearchResult` (page-1 token). Signature unchanged.
- Add to the `JiraClient` trait and impl:
  `async fn search_page(&self, jql: &str, max_results: u64, page_token: &str) -> Result<SearchResult>`.
  It builds `SearchOptions::builder().max_results(capped).next_page_token(page_token).build()`
  and calls the same `search().list(jql, &opts)` mapping path as `search` (factor the raw→
  `SearchResult` mapping into a shared helper to avoid duplication). The returned
  `SearchResult.next_page_token` carries the FOLLOWING page's token (or `None` on the last).
- Update the `SearchResult` literals in `tests/unit/models.rs` for the new field.
- Add a wiremock test for `search_page`: assert it issues the JQL with the `nextPageToken`
  param and maps the response's `nextPageToken` into the result (mirror the existing
  `search` wiremock test).

## Vertical Demo

- **Given** a wiremock server returning a page with `nextPageToken: "TOK2"`,
  **When** `client.search_page("project = X", 50, "TOK1")` runs,
  **Then** it requests with `nextPageToken=TOK1` and returns a `SearchResult` whose
  `next_page_token == Some("TOK2")` and whose `issues` are the mapped rows.
- **And** `client.search(...)` on the last page returns `next_page_token == None`; the CLI
  `mine`/`search` output is byte-unchanged.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `SearchResult` has `next_page_token: Option<String>`; `GouqiJiraClient::search` maps `raw.next_page_token` through (present when the server returns one, `None` on the last page) | test (wiremock) |
| AC2 | behavior | `JiraClient::search_page(jql, max_results, page_token)` issues the search with the `nextPageToken` query param and returns the mapped `SearchResult` carrying the next page's token | test (wiremock) |
| AC3 | constraint | `search`'s signature is unchanged and the CLI `mine`/`search_core` callers + `agent_json` output are byte-unchanged (additive field only); the raw→SearchResult mapping is shared between `search` and `search_page` (no duplication) | inspection |
| AC4 | constraint | The single-seam invariant holds: no gouqi type crosses the trait boundary; `GouqiJiraClient` stays the only construction/mapping site (ADR 0005) | inspection |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; cyclomatic ≤10 / cognitive within ceiling; a surviving mutant on the mapping/token-threading is a fail | command (clippy + fmt + comment_policy + complexity) |

## Out of scope

- Any TUI wiring (that is P3) — this slice only touches the client seam + models + their tests.
- Changing `search` to take a token (ADR 0009 chose a separate `search_page` to keep page-1
  callers unchanged).
- Threading the token through the CLI `mine`/`search` commands (CLI stays first-page).

## blocked_by

(none — independent of P1; may run before or after it. P3 depends on this.)
