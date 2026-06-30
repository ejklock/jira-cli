# Glossary

Terms and acronyms `jira-cli` docs use, defined once. Headwords are the term as-is;
only the explanation is in the doc language (English).

- **accountId** — Atlassian's opaque, immutable user identifier in Jira Cloud.
  Resolved at `setup` time from the authenticated user and stored on the instance;
  used to build the `assignee = currentUser()` listing and to attribute issues.
- **agent_json** — the curated, minified JSON output contract shared by the read
  commands, for agent/script consumers. See
  [ADR 0004](/adr/0004-agent-json-output-contract.md).
- **API token** — a Jira Cloud credential created by the user at
  id.atlassian.com. Combined with the account email as HTTP Basic auth
  (`email:token`). Stored locally in plaintext (a deliberate v1 follow-up).
- **Basic auth** — the HTTP authentication scheme Jira Cloud's REST API uses for
  API-token requests: `Authorization: Basic base64(email:token)`.
- **gouqi** — the maintained community Rust crate (`gouqi`, a fork of
  `softprops/goji`, MIT) the client layer is built on. Provides Cloud Basic auth,
  `/rest/api/3/search/jql` (V3) with `nextPageToken` pagination, and modules for
  issues/search/transitions/boards/sprints. Wrapped behind our `JiraClient` trait.
  See [ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md).
- **issue key** — the `PROJ-123` identifier of a Jira issue, unique within an
  instance.
- **JQL** — Jira Query Language. The expression language for searching issues
  (e.g. `assignee = currentUser() AND statusCategory != Done`).
- **REST v3** — the Jira Cloud Platform REST API version this tool targets
  (`/rest/api/3/...`), including the `/rest/api/3/search/jql` search endpoint.
- **SWR** — stale-while-revalidate; serve cached data immediately, then refresh in
  the background. Inherited caching pattern from the AC base.
