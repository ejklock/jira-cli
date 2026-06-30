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
