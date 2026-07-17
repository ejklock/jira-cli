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
| [0020](/issues/0020-browse-tui-chrome-i18n-parity.md) | — | browse TUI chrome i18n parity: translate footers/prompt via `t()` + pt_BR catalog | done | — |
| [0021](/issues/0021-a1-styled-adf-detail-rendering.md) | A1 | styled ADF rendering in the browse TUI detail description | done | — |
| [0022](/issues/0022-a2-keyboard-inline-link-navigation.md) | A2 | keyboard inline-link navigation in the browse TUI detail (Tab/Enter) | done | 0021 |
| [0023](/issues/0023-tui-test-hygiene-and-view-list-complexity.md) | — | TUI test hygiene (LANG_MUTEX) + view_list complexity refactor (debt) | done | — |
| [0024](/issues/0024-a4-tui-detail-comments.md) | A4 | read-only comments in the browse TUI detail (styled ADF + j/k scroll) | done | 0021, 0023 |
| [0025](/issues/0025-a3a-duedate-formatter-and-cli-get.md) | A3a | due date on the model + relative formatter + CLI get Due line | done | — |
| [0026](/issues/0026-a3b-duedate-tui-detail-and-agent-json.md) | A3b | relative Due line in the browse TUI detail + raw duedate in agent_json | done | 0025 |
| [0027](/issues/0027-h1-test-support-module-and-adf-issue-builders.md) | H1 | shared tests/unit/support.rs (ADF + Issue builders); migrate render/tui tests | done | — |
| [0028](/issues/0028-h2-consolidate-json-payload-builders.md) | H2 | centralize JSON payload builders (client + commands) into support.rs | done | 0027 |
| [0029](/issues/0029-h3-migrate-remaining-issue-fixtures.md) | H3 | migrate remaining Issue fixtures (cache/agent_json/models) to shared builder | done | 0027 |
| [0030](/issues/0030-d1-theme-header-footer.md) | D1 | theme.rs palette + identity header bar + themed footer | done | — |
| [0031](/issues/0031-d2-list-cards-due.md) | D2 | browse list as per-issue cards with colored relative due date | done | 0030 |
| [0032](/issues/0032-d3-detail-panels-scrollbar.md) | D3 | detail as stacked rounded panels + title border + clamped scrollbar | done | 0030 |
| [0033](/issues/0033-d4-contextual-footer-status-line.md) | D4 | contextual footer + thin transient status line | done | 0030 |
| [0034](/issues/0034-d5-adf-table-rendering.md) | D5 | ADF table rendering in detail/comments | done | — |
| [0035](/issues/0035-e2-401-reauth-messaging.md) | E2 | actionable 401 re-auth messaging (CLI + TUI status line) | done | 0033 |
| [0036](/issues/0036-e3-swr-first-paint-browse-entry.md) | E3 | SWR first paint on browse entry (snapshot + revalidate + guards) | done | 0033 |
| [0037](/issues/0037-b1-mouse-foundations.md) | B1 | mouse foundations: capture, wheel navigation, card click drills in | done | — |
| [0038](/issues/0038-b2a-inline-link-rendering.md) | B2a | inline body-link rendering: 'text [url]' visible token | done | — |
| [0039](/issues/0039-b2b-modifier-click-link-activation.md) | B2b | Ctrl/Cmd+click opens the '[url]' token; plain click never navigates | done | 0037, 0038 |
| [0040](/issues/0040-b3-app-managed-selection.md) | B3 | app-managed text selection in the detail (drag highlight + copy on release) | done | 0037, 0039 |
| [0041](/issues/0041-b4a-attachments-model-client-agent-json.md) | B4a | attachments on the model + client parse + agent_json array | done | — |
| [0042](/issues/0042-b4b-attachments-tui-panel.md) | B4b | Attachments panel in the browse detail (inline, link rows, footnote) | done | 0041 |
| [0043](/issues/0043-b5a-projects-client-seam.md) | B5a | projects client seam: ProjectRow model + JiraClient::list_projects | done | — |
| [0044](/issues/0044-b5b-projects-screen-tui.md) | B5b | Projects screen in the browse TUI ('p' opens, Enter drills in) | done | 0043 |
| [0045](/issues/0045-c1-comment-write-seam.md) | C1 | comment write seam: add/update/delete_comment + ADF builder + write-surface gate | done | — |
| [0046](/issues/0046-c2-non-tty-comment-command.md) | C2 | non-interactive `jira comment` (-m/stdin body, --json write result) | done | 0045 |
| [0047](/issues/0047-c3a-modal-primitive.md) | C3a | reusable modal overlay primitive (modal_area + render_modal) | done | — |
| [0048](/issues/0048-c3b-compose-post-refresh.md) | C3b | comment compose modal: 'c' opens, Ctrl+S posts, server-truth refresh | done | 0045, 0047 |
| [0049](/issues/0049-e1b-list-commands-tui-default.md) | E1b | list read commands open the browse TUI in a terminal (mine, bare, search) | done | — |
| [0050](/issues/0050-e1b-detail-commands-tui-default.md) | E1b | single-issue read commands open the detail TUI in a terminal (get, current) | done | 0049 |
| [0051](/issues/0051-c4a-comment-focus-edit.md) | C4a | comment focus axis + myself ownership + edit own comment (pre-filled compose → update) | done | 0045, 0047, 0048 |
| [0052](/issues/0052-c4b-delete-confirm-modal.md) | C4b | delete own comment via a Sim/Não confirm modal (modal button adapter → delete) | done | 0045, 0047, 0051 |
| [0053](/issues/0053-c4c-reply-mention.md) | C4c | reply to a focused comment: mentioned new comment (Jira is flat) | done | 0045, 0047, 0051 |
| [0054](/issues/0054-t1a-transition-client-seam.md) | T1a | transition client seam: list_transitions + transition_issue (GET/POST transitions endpoints) + Transition type | done | — |
| [0055](/issues/0055-t1b-transition-picker-tui.md) | T1b | transition picker TUI: 's' opens a workflow-fetched modal, Enter applies a field-free transition, server-truth refresh | done | 0054 |
| [0056](/issues/0056-c4d-detail-footer-action-hints.md) | C4d | detail footer action-key hints ([ ] focus · e edit · d delete · r reply · s status) | done | 0053, 0055 |
| [0057](/issues/0057-c4e-confirm-modal-mouse-click.md) | C4e | mouse-click activation of the delete-confirm Sim/Não buttons | done | 0052 |

## Phase 2 — browse TUI (delivered)

- `browse` interactive TUI — read-only, rebuilt fresh on an Elm/TEA + ratatui stack
  ([PRD 0002](/prd/0002-interactive-browse-tui.md), [ADR 0007](/adr/0007-browse-tui-elm-architecture.md),
  [BDR 0006](/bdr/0006-browse-tui-interactions.md)). All slices landed: B0 skeleton (0008) →
  B1 list (0009) → B2 detail (0010) → B3 search (0011) → B4 affordances (0012). Covers PRD 0002
  R1–R4. (The fork's `tui/` was never carried over; this is a fresh read-only build, not a revival.)
- **In-loop async delivery + in-TUI pagination** (the former PRD 0002 open questions) are
  now sliced as P1 (0017, [ADR 0008](/adr/0008-browse-tui-async-event-loop.md)), P2 (0018)
  and P3 (0019, [ADR 0009](/adr/0009-tui-list-pagination.md)).

## active-collab-cli feature-parity program (read-only first)

Goal: total feature parity with the fork base `active-collab-cli`, adapted to Jira's
specifics ([ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md)). Direction chosen:
**read-only now, writes later.** Group A (below) is planned; Groups B and C are parked
behind their own decision records.

**Group A — read-only, no recorded-decision conflict (planned):**

- **A1** — [0021](/issues/0021-a1-styled-adf-detail-rendering.md): styled ADF rendering in
  the browse TUI detail ([ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md)). **open**
- **A2** — [0022](/issues/0022-a2-keyboard-inline-link-navigation.md): keyboard inline-link
  navigation in the detail (Tab cycle, Enter open) over A1's retained `href`
  ([ADR 0011](/adr/0011-keyboard-inline-link-navigation-browse-detail.md)). **open**
- **A3** — relative due-date formatting from the issue `duedate`
  ([ADR 0013](/adr/0013-relative-due-date-rendering.md)), sliced into
  [0025](/issues/0025-a3a-duedate-formatter-and-cli-get.md) (A3a: model + formatter + CLI `get`)
  and [0026](/issues/0026-a3b-duedate-tui-detail-and-agent-json.md) (A3b: TUI detail + agent_json).
  **done**

**Group A is complete** (A1 · A2 · A3 · A4 all delivered). Known follow-up debt: a test-support
consolidation slice (a shared `Issue` test-builder) to DRY the per-module fixtures that trip the
duplication gate on every `Issue`-struct-touching change (observation 55).
- **A4** — [0024](/issues/0024-a4-tui-detail-comments.md): read-only comments in the TUI detail,
  styled via `adf_to_rich`, scrollable with `j/k` + `↑/↓`
  ([ADR 0012](/adr/0012-comments-in-browse-tui-detail.md)). **done**

**Total-parity program unblocked (2026-07-06):** [PRD 0003](/prd/0003-active-collab-parity.md)
+ Constitution Amendment 1 reopened the former Groups B and C. Execution order
**D → E → B → C** (plan `ac-parity-program`):

- **Group D — visual design parity** ([ADR 0014](/adr/0014-tui-visual-design-system.md),
  [BDR 0007](/bdr/0007-tui-visual-design-behaviors.md)): slices D1–D5 = issues
  [0030](/issues/0030-d1-theme-header-footer.md)–[0034](/issues/0034-d5-adf-table-rendering.md). **done**
  Known hardening follow-up: a tableHeader-cell-with-marks mutation test (D5 review observation).
- **Group E — behavior parity** (PRD 0003 R-E1..R-E3): **done**, including the E1
  correction. **E1** (bare TTY → `mine`) routed to `mine` but stopped
  at the printed table; the fork opens the TUI. The TTY/agent duality is
  formalized and applied to all read commands in **E1b**
  ([ADR 0025](/adr/0025-tty-interactive-default-read-commands.md),
  [BDR 0016](/bdr/0016-interactive-default-read-commands.md), issues
  [0049](/issues/0049-e1b-list-commands-tui-default.md) +
  [0050](/issues/0050-e1b-detail-commands-tui-default.md)) — **done**
  (mine/bare/search/get/current all open the browse TUI seeded to the right
  screen in an interactive terminal; agent mode prints byte-unchanged);
  **E2** = [0035](/issues/0035-e2-401-reauth-messaging.md) **done**;
  **E3** = [0036](/issues/0036-e3-swr-first-paint-browse-entry.md)
  ([ADR 0016](/adr/0016-swr-first-paint-browse-entry.md),
  [BDR 0008](/bdr/0008-browse-entry-swr-behaviors.md)) **done**.
- **Group B — interaction parity** (PRD 0003 R-B1..R-B5), sliced B1→B5: **done.**
  **B1** = [0037](/issues/0037-b1-mouse-foundations.md) mouse foundations
  ([ADR 0017](/adr/0017-mouse-support-browse-tui.md),
  [BDR 0009](/bdr/0009-browse-mouse-interactions.md)) **done**;
  **B2** inline body links + Ctrl/Cmd+click, sliced
  [0038](/issues/0038-b2a-inline-link-rendering.md) +
  [0039](/issues/0039-b2b-modifier-click-link-activation.md)
  ([ADR 0018](/adr/0018-inline-body-links-modifier-click.md),
  [BDR 0010](/bdr/0010-inline-body-link-behaviors.md)) **done** —
  links before selection, mirroring the fork's dependency arc (the detail
  hit geometry selection reuses); **B3** = [0040](/issues/0040-b3-app-managed-selection.md)
  app-managed text selection + clipboard
  ([ADR 0019](/adr/0019-app-managed-text-selection.md),
  [BDR 0011](/bdr/0011-detail-text-selection-behaviors.md)) **done**;
  **B4** attachments ([ADR 0020](/adr/0020-issue-attachments-detail-panel.md),
  [BDR 0012](/bdr/0012-attachments-behaviors.md)), sliced
  [0041](/issues/0041-b4a-attachments-model-client-agent-json.md) (data + agent_json) +
  [0042](/issues/0042-b4b-attachments-tui-panel.md) (TUI panel) **done**;
  **B5** Projects→Issues axis ([ADR 0021](/adr/0021-projects-axis-browse.md),
  [BDR 0013](/bdr/0013-projects-axis-behaviors.md)), sliced
  [0043](/issues/0043-b5a-projects-client-seam.md) (client seam) +
  [0044](/issues/0044-b5b-projects-screen-tui.md) (TUI screen) **done**.
- **Group C — comment writes** ([ADR 0015](/adr/0015-comment-write-enablement.md)),
  sliced C1→C4: **C1** = [0045](/issues/0045-c1-comment-write-seam.md) write seam
  ([ADR 0022](/adr/0022-comment-write-seam.md)) **done**; **C2** =
  [0046](/issues/0046-c2-non-tty-comment-command.md) non-TTY `jira comment`
  ([ADR 0023](/adr/0023-non-tty-comment-command.md),
  [BDR 0014](/bdr/0014-non-interactive-comment-behaviors.md)) **done**;
  **C3** TUI compose ([ADR 0024](/adr/0024-modal-overlay-compose.md),
  [BDR 0015](/bdr/0015-comment-compose-behaviors.md)), sliced
  [0047](/issues/0047-c3a-modal-primitive.md) (modal primitive) +
  [0048](/issues/0048-c3b-compose-post-refresh.md) (compose + POST +
  refresh) **done**; **C4** comment actions
  ([ADR 0026](/adr/0026-comment-edit-delete-reply-focus.md),
  [BDR 0017](/bdr/0017-comment-action-behaviors.md)) — a `[`/`]` comment-focus
  axis + `myself` ownership, sliced
  [0051](/issues/0051-c4a-comment-focus-edit.md) (focus + edit own) +
  [0052](/issues/0052-c4b-delete-confirm-modal.md) (delete via Sim/Não confirm) +
  [0053](/issues/0053-c4c-reply-mention.md) (reply = mentioned new comment) **done**.

## Parked (not in the parity program)

- Other writes: create/edit issue, log work (out — each would need its own
  amendment + ADR). Status transitions entered scope via Constitution
  Amendment 2 ([ADR 0027](/adr/0027-status-transition-write-enablement.md),
  issues 0054/0055).
- Jira Server / Data Center (REST v2 / PAT) adapter.
- Secret-at-rest encryption / OS keychain; native release binaries.
