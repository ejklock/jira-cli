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
