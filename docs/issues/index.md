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

## Parked (Phase 2 — not yet sliced)

- `browse` interactive TUI (re-activate the dormant `tui/` module).
- Write operations: comment, transition status, log work.
- Jira Server / Data Center (REST v2 / PAT) adapter.
- Secret-at-rest encryption / OS keychain; native release binaries.
