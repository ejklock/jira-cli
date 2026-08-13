# ADRs

Architectural/implementation decisions for `jira-cli`. Append-only: supersede,
never rewrite. Trace up to [PRD 0001](/prd/0001-jira-cloud-read-cli.md) and the
[Constitution](/constitution.md).

## Active

* [0001 — Fork active-collab-cli and swap the API layer to Jira Cloud](0001-fork-active-collab-cli-swap-api.md) - Accepted
* [0002 — Jira Cloud only for v1 (REST v3 + Basic auth email+API token)](0002-jira-cloud-only-basic-auth.md) - Accepted
* [0003 — Issue identity = (instance_name, issue_key); cache keyed on it](0003-issue-identity-and-cache-key.md) - Accepted
* [0004 — Curated, minified agent_json output contract (--json), inherited and re-shaped for Jira](0004-agent-json-output-contract.md) - Accepted
* [0005 — Build the Jira client on the gouqi crate, behind our own client trait](0005-jira-client-on-gouqi-behind-trait.md) - Accepted
* [0006 — i18n interpolation contract: translate the template, then substitute](0006-i18n-interpolation-contract.md) - Accepted
* [0007 — Browse TUI: a read-only Elm/TEA shell over the existing domain core (ratatui + crossterm)](0007-browse-tui-elm-architecture.md) - Accepted
* [0008 — Browse TUI: realize the async select event loop (EventStream + mpsc), retiring the block_in_place shell](0008-browse-tui-async-event-loop.md) - Accepted
* [0009 — Browse TUI list pagination: expose the search next_page_token and load more pages on demand](0009-tui-list-pagination.md) - Accepted
* [0010 — Styled ADF rendering in the browse TUI detail (neutral rich model + ratatui mapping)](0010-styled-adf-rendering-browse-tui-detail.md) - Accepted
* [0011 — Keyboard inline-link navigation in the browse TUI detail (Tab to cycle, Enter to open)](0011-keyboard-inline-link-navigation-browse-detail.md) - Accepted
* [0012 — Read-only comments in the browse TUI detail (styled ADF + j/k scroll)](0012-comments-in-browse-tui-detail.md) - Accepted
* [0013 — Relative due-date rendering (stdlib date math, English-source i18n keys)](0013-relative-due-date-rendering.md) - Accepted
* [0014 — TUI visual design system — port the vibrant-dashboard look from active-collab-cli](0014-tui-visual-design-system.md) - Accepted
* [0015 — Comment write enablement — POST/PUT/DELETE on the Jira comment endpoints only](0015-comment-write-enablement.md) - Accepted
* [0016 — First-paint-from-cache SWR on browse entry (task-list snapshot)](0016-swr-first-paint-browse-entry.md) - Accepted
* [0017 — Mouse support in the browse TUI (capture lifecycle, wheel, card click)](0017-mouse-support-browse-tui.md) - Accepted
* [0018 — Inline body links ('text [url]') with modifier-gated click activation](0018-inline-body-links-modifier-click.md) - Accepted
* [0019 — App-managed text selection in the detail (logical-coordinate drag + clipboard)](0019-app-managed-text-selection.md) - Accepted
* [0020 — Issue attachments — curated model field, agent_json array, and an inline Attachments panel in the detail](0020-issue-attachments-detail-panel.md) - Accepted
* [0021 — Projects axis in the browse TUI — 'p' opens a Projects screen; a project drills into its issues](0021-projects-axis-browse.md) - Accepted
* [0022 — Comment write seam — gouqi write verbs behind the JiraClient trait, plain-text→ADF builder, comment author identity](0022-comment-write-seam.md) - Accepted
* [0023 — A non-interactive `comment` command posts a comment as the logged-in user, extending the agent --json contract to a write result](0023-non-tty-comment-command.md) - Accepted
* [0024 — A reusable centered modal overlay hosts the comment compose (modal-first, no inline detour); server-truth refresh after every mutation](0024-modal-overlay-compose.md) - Accepted
* [0025 — TTY = interactive-by-default for read commands (agent mode prints)](0025-tty-interactive-default-read-commands.md) - Accepted
* [0026 — Comment actions in the browse detail — a comment-focus axis gates edit/delete (own only) + reply (any); edit reuses the compose modal, delete uses the modal's confirm buttons, reply posts a new comment; every mutation is server-truth](0026-comment-edit-delete-reply-focus.md) - Accepted
* [0027 — Status transition write enablement — GET/POST on the Jira transitions endpoints only, field-free transitions in v1](0027-status-transition-write-enablement.md) - Accepted
* [0028 — The agent skill is served by a `jira skill` CLI command; per-harness integrations are thin pointers to it](0028-agent-skill-served-by-jira-skill-command.md) - Accepted
* [0029 — Attachments: authenticated download seam, --download-attachments, and external image viewer](0029-attachments-authenticated-download-seam-download-attachments-and-external-image-viewer.md) - Accepted
* [0030 — Rename the agent skill identifier from `jira` to `jira-ticket`](0030-rename-agent-skill-to-jira-ticket.md) - Accepted
