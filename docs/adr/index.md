# ADRs

Architectural/implementation decisions for `jira-cli`. Append-only: supersede,
never rewrite. Trace up to [PRD 0001](/prd/0001-jira-cloud-read-cli.md) and the
[Constitution](/constitution.md).

| # | Title | Status |
|---|---|---|
| [0001](/adr/0001-fork-active-collab-cli-swap-api.md) | Fork active-collab-cli, swap the API layer to Jira Cloud | Accepted |
| [0002](/adr/0002-jira-cloud-only-basic-auth.md) | Jira Cloud only for v1 (REST v3 + Basic auth email+API token) | Accepted |
| [0003](/adr/0003-issue-identity-and-cache-key.md) | Issue identity = (instance_name, issue_key); cache keyed on it | Accepted |
| [0004](/adr/0004-agent-json-output-contract.md) | Curated, minified agent_json output contract (inherited) | Accepted |
| [0005](/adr/0005-jira-client-on-gouqi-behind-trait.md) | Build the Jira client on the gouqi crate, behind our own client trait | Accepted |
| [0006](/adr/0006-i18n-interpolation-contract.md) | i18n interpolation contract: translate the template, then substitute | Accepted |
| [0007](/adr/0007-browse-tui-elm-architecture.md) | Browse TUI: read-only Elm/TEA shell over the domain core (ratatui + crossterm) | Accepted |
| [0008](/adr/0008-browse-tui-async-event-loop.md) | Browse TUI: realize the async select event loop (EventStream + mpsc), retiring block_in_place | Accepted |
| [0009](/adr/0009-tui-list-pagination.md) | Browse TUI list pagination: expose search next_page_token + load more on demand | Accepted |
| [0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md) | Styled ADF rendering in the browse TUI detail (neutral rich model + ratatui mapping) | Accepted |
| [0011](/adr/0011-keyboard-inline-link-navigation-browse-detail.md) | Keyboard inline-link navigation in the browse TUI detail (Tab cycle, Enter open) | Accepted |
| [0012](/adr/0012-comments-in-browse-tui-detail.md) | Read-only comments in the browse TUI detail (styled ADF + j/k scroll) | Accepted |
| [0013](/adr/0013-relative-due-date-rendering.md) | Relative due-date rendering (stdlib date math, English-source i18n keys) | Accepted |
| [0014](/adr/0014-tui-visual-design-system.md) | TUI visual design system — vibrant-dashboard parity (theme, cards, panels, footer/status, ADF tables) | Accepted |
| [0015](/adr/0015-comment-write-enablement.md) | Comment write enablement — POST/PUT/DELETE on the comment endpoints only | Accepted |
| [0016](/adr/0016-swr-first-paint-browse-entry.md) | First-paint-from-cache SWR on browse entry (task-list snapshot) | Accepted |
| [0017](/adr/0017-mouse-support-browse-tui.md) | Mouse support in the browse TUI (capture lifecycle, wheel, card click) | Accepted |
| [0018](/adr/0018-inline-body-links-modifier-click.md) | Inline body links ('text [url]') with modifier-gated click activation | Accepted |
| [0019](/adr/0019-app-managed-text-selection.md) | App-managed text selection in the detail (logical-coordinate drag + clipboard) | Accepted |
| [0020](/adr/0020-issue-attachments-detail-panel.md) | Issue attachments — curated model field, agent_json array, inline Attachments panel | Accepted |
| [0021](/adr/0021-projects-axis-browse.md) | Projects axis in the browse TUI — 'p' opens Projects; a project drills into its issues | Accepted |
| [0022](/adr/0022-comment-write-seam.md) | Comment write seam — gouqi write verbs behind JiraClient, plain-text→ADF builder, author identity | Accepted |
| [0023](/adr/0023-non-tty-comment-command.md) | Non-interactive `comment` command — -m/stdin body, --json write result, structural identity | Accepted |
| [0024](/adr/0024-modal-overlay-compose.md) | Reusable centered modal overlay + comment compose (modal-first); server-truth refresh | Accepted |
| [0025](/adr/0025-tty-interactive-default-read-commands.md) | TTY = interactive-by-default for read commands (mine/bare/search/get/current → browse TUI; agent mode prints) | Accepted |
| [0026](/adr/0026-comment-edit-delete-reply-focus.md) | Comment edit/delete/reply via a comment-focus axis — own-gated edit/delete, reply-as-mentioned-new-comment, server-truth | Accepted |
| [0027](/adr/0027-status-transition-write-enablement.md) | Status transition write enablement — GET/POST the transitions endpoints only, field-free transitions in v1, picker modal, server-truth | Accepted |
| [0028](/adr/0028-agent-skill-served-by-jira-skill-command.md) | Agent skill served by a `jira skill` command; per-harness integrations are thin pointers; install-skill.sh with `--scope project\|global` (ports ActiveCollab ADR 0057-0059) | Accepted |
