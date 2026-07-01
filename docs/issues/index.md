# Issues

Vertical slices for `jira-cli` v1 (the Jira Cloud read CLI), tracing to
[PRD 0001](/prd/0001-jira-cloud-read-cli.md) and the ADRs/BDRs each row links.
Each slice delivers one user-observable behavior end-to-end, demoable on a real
Jira Cloud instance. J0 is the walking skeleton; the rest stack on it.

| # | Slice | Title | Status | Blocked by |
|---|---|---|---|---|
| [0001](/issues/0001-j0-skeleton-setup-get.md) | J0 | walking skeleton: fork scaffold + setup + get | open | — |
| [0002](/issues/0002-j1-local-first-cache.md) | J1 | local-first cache read: offline get + --refresh | open | 0001 |
| [0003](/issues/0003-j2-current-from-branch.md) | J2 | current: issue from the git branch | open | 0001 |
| [0004](/issues/0004-j3-mine-list.md) | J3 | mine/list: open issues assigned to me (JQL) | open | 0001 |
| [0005](/issues/0005-j4-search-jql.md) | J4 | search: arbitrary JQL | open | 0004 |
| [0006](/issues/0006-j5-i18n.md) | J5 | i18n: English + Brazilian Portuguese output | open | 0001 |
| [0007](/issues/0007-i18n-interpolation-fix.md) | — | i18n: fix format-then-translate (interpolated chrome) | open | 0006 |
| [0008](/issues/0008-b0-browse-skeleton.md) | B0 | browse TUI walking skeleton: command + raw-mode shell + quit | open | 0001 |
| [0009](/issues/0009-b1-browse-list.md) | B1 | browse TUI: navigable issue list (mine) | done | 0008 |
| [0010](/issues/0010-b2-browse-detail.md) | B2 | browse TUI: issue detail on Enter (cache-or-fetch) | done | 0009 |
| [0011](/issues/0011-b3-browse-search.md) | B3 | browse TUI: interactive JQL search | done | 0009 |
| [0012](/issues/0012-b4-browse-affordances.md) | B4 | browse TUI: read affordances (open link, copy key) | done | 0009 |
| [0013](/issues/0013-ordered-list-numbered-prefix.md) | — | ADF orderedList renders a numbered prefix (debt) | done | — |
| [0014](/issues/0014-i18n-human-detail-field-labels.md) | — | i18n: translate human-render field labels (get + browse detail) | done | — |
| [0015](/issues/0015-split-tui-into-submodule.md) | — | split `src/tui.rs` into a `src/tui/` submodule (model/view/shell) | done | — |
| [0016](/issues/0016-consolidate-lang-mutex.md) | — | consolidate the 4 per-module `LANG_MUTEX` into one crate-wide test lock | done | — |
| [0017](/issues/0017-p1-async-event-loop.md) | P1 | browse TUI async event loop (EventStream + mpsc), retire block_in_place | done | — |
| [0018](/issues/0018-p2-pagination-client-seam.md) | P2 | pagination client seam: `SearchResult.next_page_token` + `search_page` | done | — |
| [0019](/issues/0019-p3-tui-pagination-wiring.md) | P3 | browse TUI pagination wiring: load-more appends the next page | done | 0017, 0018 |

## Phase 2 — browse TUI (delivered)

- `browse` interactive TUI — read-only, rebuilt fresh on an Elm/TEA + ratatui stack
  ([PRD 0002](/prd/0002-interactive-browse-tui.md), [ADR 0007](/adr/0007-browse-tui-elm-architecture.md),
  [BDR 0006](/bdr/0006-browse-tui-interactions.md)). All slices landed: B0 skeleton (0008) →
  B1 list (0009) → B2 detail (0010) → B3 search (0011) → B4 affordances (0012). Covers PRD 0002
  R1–R4. (The fork's `tui/` was never carried over; this is a fresh read-only build, not a revival.)
- **In-loop async delivery + in-TUI pagination** (the former PRD 0002 open questions) are
  now sliced as P1 (0017, [ADR 0008](/adr/0008-browse-tui-async-event-loop.md)), P2 (0018)
  and P3 (0019, [ADR 0009](/adr/0009-tui-list-pagination.md)).

## Parked (Phase 2 — not yet sliced)

- Write operations: comment, transition status, log work.
- Jira Server / Data Center (REST v2 / PAT) adapter.
- Secret-at-rest encryption / OS keychain; native release binaries.
