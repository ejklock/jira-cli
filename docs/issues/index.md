# Issues

Vertical slices for `jira-cli` v1 (the Jira Cloud read CLI), tracing to
[PRD 0001](/prd/0001-jira-cloud-read-cli.md) and the ADRs/BDRs each row links.
Each slice delivers one user-observable behavior end-to-end, demoable on a real
Jira Cloud instance. J0 is the walking skeleton; the rest stack on it.

## Open

* [0001 — J0 — walking skeleton: fork scaffold + setup + get PROJ-123 end-to-end](0001-j0-skeleton-setup-get.md) - open
* [0002 — J1 — local-first cache read: offline get + --refresh](0002-j1-local-first-cache.md) - open
* [0003 — J2 — current: issue from the git branch](0003-j2-current-from-branch.md) - open
* [0004 — J3 — mine/list: open issues assigned to me (JQL)](0004-j3-mine-list.md) - open
* [0005 — J4 — search: arbitrary JQL](0005-j4-search-jql.md) - open
* [0006 — J5 — i18n: English + Brazilian Portuguese CLI output](0006-j5-i18n.md) - open
* [0007 — i18n: fix format-then-translate so interpolated chrome translates](0007-i18n-interpolation-fix.md) - open
* [0008 — B0 — browse TUI walking skeleton: command + raw-mode shell + quit](0008-b0-browse-skeleton.md) - open
* [0013 — ADF orderedList renders a numbered prefix (latent debt fix)](0013-ordered-list-numbered-prefix.md) - open
* [0058 — SK1 — `jira skill` command: pure src/skill.rs registry + skill_output, include_str! the canonical SKILL.md, 'skill' joins KNOWN_COMMANDS](0058-sk1-jira-skill-command.md) - open
* [0059 — SK2 — install-skill.sh: thin per-harness pointers to `jira skill jira` with --harness <name>|all and --scope project|global; fix install.sh fork bug](0059-sk2-install-skill-installer.md) - open
* [0060 — D2a — attachment download seam: download_attachment on JiraClient with same-origin guard](0060-d2a-attachment-download-seam-download-attachment-on-jiraclient-with-same-origin-guard.md) - Proposed
* [0061 — D2b — jira get --download-attachments writes every attachment to the config downloads dir](0061-d2b-jira-get-download-attachments-writes-every-attachment-to-the-config-downloads-dir.md) - Proposed
* [0062 — D1 — browse TUI opens an image attachment in the OS viewer](0062-d1-browse-tui-opens-an-image-attachment-in-the-os-viewer.md) - Proposed

## Closed

* [0009 — B1 — browse TUI: navigable issue list (mine)](0009-b1-browse-list.md) - done
* [0010 — B2 — browse TUI: issue detail on Enter (cache-or-fetch, scroll, back)](0010-b2-browse-detail.md) - done
* [0011 — B3 — browse TUI: interactive JQL search](0011-b3-browse-search.md) - done
* [0012 — B4 — browse TUI: read affordances (open link, copy key)](0012-b4-browse-affordances.md) - done
* [0014 — i18n: translate human-render field labels (get + browse detail)](0014-i18n-human-detail-field-labels.md) - done
* [0015 — Split src/tui.rs into a src/tui/ submodule (model / view / shell)](0015-split-tui-into-submodule.md) - done
* [0016 — Consolidate the 4 per-module LANG_MUTEX statics into one crate-wide test lock](0016-consolidate-lang-mutex.md) - done
* [0017 — P1 — browse TUI async event loop (EventStream + mpsc), retire block_in_place](0017-p1-async-event-loop.md) - done
* [0018 — P2 — pagination client seam: expose SearchResult.next_page_token + JiraClient::search_page](0018-p2-pagination-client-seam.md) - done
* [0019 — P3 — browse TUI pagination wiring: load-more appends the next page](0019-p3-tui-pagination-wiring.md) - done
* [0020 — browse TUI chrome i18n parity — translate remaining footers/prompt via t() + pt_BR catalog](0020-browse-tui-chrome-i18n-parity.md) - done
* [0021 — A1 — styled ADF rendering in the browse TUI detail description](0021-a1-styled-adf-detail-rendering.md) - done
* [0022 — A2 — keyboard inline-link navigation in the browse TUI detail](0022-a2-keyboard-inline-link-navigation.md) - done
* [0023 — TUI test hygiene + view_list complexity refactor (debt)](0023-tui-test-hygiene-and-view-list-complexity.md) - done
* [0024 — A4 — read-only comments in the browse TUI detail (styled ADF + j/k scroll)](0024-a4-tui-detail-comments.md) - done
* [0025 — A3a — due date on the model + relative formatter + CLI get Due line](0025-a3a-duedate-formatter-and-cli-get.md) - done
* [0026 — A3b — relative Due line in the browse TUI detail + raw duedate in agent_json](0026-a3b-duedate-tui-detail-and-agent-json.md) - done
* [0027 — H1 — shared tests/unit/support.rs (ADF + Issue builders), migrate render + tui tests](0027-h1-test-support-module-and-adf-issue-builders.md) - done
* [0028 — H2 — centralize the JSON payload builders (client + commands) into support.rs](0028-h2-consolidate-json-payload-builders.md) - done
* [0029 — H3 — migrate the remaining Issue fixtures (cache/agent_json/models) to the shared builder](0029-h3-migrate-remaining-issue-fixtures.md) - done
* [0030 — D1 — theme.rs palette + identity header bar + themed footer](0030-d1-theme-header-footer.md) - done
* [0031 — D2 — browse list as per-issue cards with colored relative due date](0031-d2-list-cards-due.md) - done
* [0032 — D3 — detail as stacked rounded panels + title border + clamped scrollbar](0032-d3-detail-panels-scrollbar.md) - done
* [0033 — D4 — contextual footer + thin transient status line](0033-d4-contextual-footer-status-line.md) - done
* [0034 — D5 — ADF table rendering in detail/comments](0034-d5-adf-table-rendering.md) - done
* [0035 — E2 — actionable 401 re-auth messaging (CLI + TUI status line)](0035-e2-401-reauth-messaging.md) - done
* [0036 — E3 — SWR first paint on browse entry (snapshot + revalidate + guards)](0036-e3-swr-first-paint-browse-entry.md) - done
* [0037 — B1 — mouse foundations: capture lifecycle, wheel navigation, card click drills in](0037-b1-mouse-foundations.md) - done
* [0038 — B2a — inline body-link rendering: 'text [url]' with a visible link-styled token](0038-b2a-inline-link-rendering.md) - done
* [0039 — B2b — Ctrl/Cmd+click opens the '[url]' token; plain click never navigates](0039-b2b-modifier-click-link-activation.md) - done
* [0040 — B3 — app-managed text selection in the detail (drag highlight + copy on release)](0040-b3-app-managed-selection.md) - done
* [0041 — B4a — attachments on the model + client parse + agent_json array](0041-b4a-attachments-model-client-agent-json.md) - done
* [0042 — B4b — Attachments panel in the browse detail (inline, link rows, footnote)](0042-b4b-attachments-tui-panel.md) - done
* [0043 — B5a — projects client seam: ProjectRow model + JiraClient::list_projects](0043-b5a-projects-client-seam.md) - done
* [0044 — B5b — Projects screen in the browse TUI ('p' opens, Enter drills into the project's issues)](0044-b5b-projects-screen-tui.md) - done
* [0045 — C1 — comment write seam: add/update/delete_comment on JiraClient, ADF builder, author accountId, write-surface gate](0045-c1-comment-write-seam.md) - done
* [0046 — C2 — non-interactive `jira comment` command (-m or stdin body, --json write result)](0046-c2-non-tty-comment-command.md) - done
* [0047 — C3a — reusable modal overlay primitive (modal_area + render_modal, dimmed backdrop, themed box)](0047-c3a-modal-primitive.md) - done
* [0048 — C3b — comment compose through the modal: 'c' opens, Ctrl+S posts, server-truth refresh](0048-c3b-compose-post-refresh.md) - done
* [0049 — E1b — list read commands open the browse TUI in a terminal (mine, bare, search)](0049-e1b-list-commands-tui-default.md) - done
* [0050 — E1b — single-issue read commands open the detail TUI in a terminal (get, current)](0050-e1b-detail-commands-tui-default.md) - done
* [0051 — C4a — comment focus axis + myself ownership + edit own comment (pre-filled compose → update_comment, server-truth refresh)](0051-c4a-comment-focus-edit.md) - done
* [0052 — C4b — delete own comment via a Sim/Não confirm modal (the modal primitive's button adapter → delete_comment, server-truth refresh)](0052-c4b-delete-confirm-modal.md) - done
* [0053 — C4c — reply to a focused comment: a new comment seeded with an @mention of the author (Jira is flat → reply = new top-level comment)](0053-c4c-reply-mention.md) - done
* [0054 — T1a — transition client seam: list_transitions + transition_issue on the JiraClient trait (GET/POST the Jira transitions endpoints), with a Transition domain type](0054-t1a-transition-client-seam.md) - done
* [0055 — T1b — transition picker TUI: 's' opens a modal fetched from the workflow, Enter applies a field-free transition, the detail reloads from the server](0055-t1b-transition-picker-tui.md) - done
* [0056 — C4d — detail footer action-key hints: advertise the comment-focus/edit/delete/reply and status keys in the browse-detail footer](0056-c4d-detail-footer-action-hints.md) - done
* [0057 — C4e — mouse-click activation of the delete-confirm Sim/Não buttons: wire the modal's ButtonTarget click geometry (ADR 0024 §2d) into shell mouse resolution](0057-c4e-confirm-modal-mouse-click.md) - done
