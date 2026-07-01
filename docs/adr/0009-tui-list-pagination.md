---
type: ADR
title: "Browse TUI list pagination: expose the search next_page_token and load more pages on demand"
description: Expose gouqi's already-available nextPageToken through the domain SearchResult and add a JiraClient::search_page(jql, max_results, page_token) method, so the browse TUI can fetch subsequent pages of a JQL result and append them to the list on an explicit user action (load-more), token-based (V3), read-only. The CLI mine/search path (first page + --limit) is unchanged.
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, phase2, pagination, client, read]
timestamp: 2026-06-30T00:00:00Z
---

# 0009. Browse TUI list pagination: expose next_page_token + load more on demand

## Context

[PRD 0002](/prd/0002-interactive-browse-tui.md) parked in-TUI paging as an open question:
"the first page + `--limit` is the v1 contract; infinite scroll/pagination inside the TUI
is deferred." The browse list therefore shows only the first `DEFAULT_SEARCH_LIMIT` rows
of the `mine`/JQL result, with no way to reach the rest.

A feasibility spike against the vendored gouqi 0.20 source (spike-gouqi-pagination)
confirmed token pagination is fully supported and clean:

- `gouqi::rep::SearchResults` carries `next_page_token: Option<String>` (rep.rs:1007) and
  `is_last_page: Option<bool>` alongside `issues`/`total`/`start_at`.
- `SearchOptionsBuilder::next_page_token(&str)` (builder.rs:175) sets the `nextPageToken`
  query param for the V3 `/search/jql` endpoint; gouqi's own `iter`/`stream` already use
  it (`search.rs:329` prefers the token, falling back to `start_at` offset for legacy).

Our domain `SearchResult` (src/models.rs) currently **drops** the token — it maps only
`issues`/`total`/`is_last_page` — so nothing downstream can request the next page. The gap
is small and confined to the client seam.

## Decision

Expose the page token through the domain boundary and add one client method to fetch a
subsequent page; wire the browse TUI to append pages on an explicit **load-more** action.
The single outbound-network seam ([ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md))
stays the only place a gouqi type is touched.

1. **Domain `SearchResult` gains `next_page_token: Option<String>`.** `GouqiJiraClient::search`
   maps `raw.next_page_token` through (additive field; `is_last_page`/`total` unchanged).
   The field is `None` on the last page. `agent_json`/CLI render are unaffected (they never
   read it).
2. **New trait method `JiraClient::search_page(jql, max_results, page_token) -> Result<SearchResult>`.**
   It builds `SearchOptions::builder().max_results(capped).next_page_token(token)` and calls
   the same `search().list()` path as `search`, returning the mapped `SearchResult` (its own
   `next_page_token` carrying the following page's token). Page 1 keeps using `search`
   (signature **unchanged**), so the CLI `mine`/`search` callers are untouched — pagination
   is confined to the TUI. `GouqiJiraClient` is the only impl; there is no mock to update
   (tests hit the real client via wiremock).
3. **Load-more is an explicit user action, not auto-infinite-scroll.** In the browse list,
   when the current result has a `next_page_token`, a key (`n` / "load more") — and reaching
   the last row with a pending token — emits a `Cmd::LoadMore`; its result **appends** to the
   existing rows (selection preserved) and updates the stored token. This keeps navigation
   pure (nav never triggers hidden I/O by itself beyond the explicit trigger) and the fetch
   an observable action. The list footer shows a "more" affordance while a token is pending.
4. **The TUI Model tracks the paging cursor.** The list Model carries the active `jql` (so
   the next page repeats the same query) and the current `next_page_token`. A fresh
   list/search (`ListLoaded`) resets the cursor from the new result; `LoadMore` advances it.
5. **Read-only + bounded.** No "load all" prefetch (unbounded); each load-more fetches one
   more page of `DEFAULT_SEARCH_LIMIT`. Rides the async loop
   ([ADR 0008](/adr/0008-browse-tui-async-event-loop.md)) — `LoadMore` is another spawned
   effect returning `MoreLoaded`/`LoadFailed`.

## Alternatives considered

- **Auto infinite-scroll** (fetch the next page automatically when the selection nears the
  end). Rejected for v1: couples pure navigation to hidden I/O, complicating reasoning and
  the demo; the explicit load-more is simpler and still reaches every row. Can be layered
  later on the same primitive.
- **`startAt` offset paging** instead of the token. Rejected: the V3 `/search/jql` endpoint
  is token-based; gouqi prefers `next_page_token` and only falls back to `start_at` for the
  legacy endpoint. Using the token matches the endpoint the client is pinned to (V3).
- **A "load all" that fetches every page up front.** Rejected: unbounded memory/latency for
  large results; defeats the point of paging. `--limit`/one-page-at-a-time stays the contract.
- **Thread an optional token into the existing `search` signature.** Rejected in favor of a
  separate `search_page`: keeps the page-1 callers (CLI `mine`/`search`) byte-unchanged and
  makes the two intents explicit at the call site.

## Consequences

**Positive:**

- The browse list can reach beyond the first page — the last user-facing gap in PRD 0002's
  list requirement — with a bounded, read-only, explicit action.
- The change is confined: one additive `SearchResult` field + one new client method behind
  the existing trait seam; the CLI path and `agent_json` contract are untouched.
- Reuses gouqi's native token support (no hand-rolled pagination) and the async loop's
  spawn/channel plumbing (no new shell machinery).

**Accepted trade-offs:**

- `SearchResult` gains a field, so its literals in tests and the client mapper update
  (additive, mechanical). Serialization is additive (`Option`, defaults to absent).
- The TUI Model grows a small paging cursor (`jql` + `next_page_token`); the transitions
  stay pure and unit-tested (`LoadMore`/`MoreLoaded` in `update`), the append is a pure
  state change.
- Load-more is manual (a key), not automatic; reaching a very deep row takes repeated
  actions. Accepted for v1; auto-scroll is a later layer on the same primitive.

## Related

- ADR: [/adr/0005-jira-client-on-gouqi-behind-trait.md](/adr/0005-jira-client-on-gouqi-behind-trait.md) (the single client seam this extends)
- ADR: [/adr/0008-browse-tui-async-event-loop.md](/adr/0008-browse-tui-async-event-loop.md) (LoadMore rides the async loop)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md) (open question: in-TUI paging)
- BDR: [/bdr/0006-browse-tui-interactions.md](/bdr/0006-browse-tui-interactions.md) (S8 load-more)
