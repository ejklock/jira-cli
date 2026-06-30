# Context

Domain and module vocabulary for `jira-cli`. One home per concept; other docs link
here rather than redefine. See also the [Glossary](/context/glossary.md) for terms
and acronyms.

## Domain

- **Instance** — a configured Jira Cloud site (`name`, `base_url`, `email`,
  `token`, `account_id`). Selected explicitly (`--instance`) or inferred.
- **Issue** — a Jira work item, identified across the tool by the pair
  `(instance_name, issue_key)`. See
  [ADR 0003](/adr/0003-issue-identity-and-cache-key.md).
- **Issue key** — the human-facing `PROJ-123` identifier, globally unique within
  an instance.
- **JQL** — Jira Query Language; the query string the `mine`/`list` and `search`
  commands send to the Cloud search API.

## Modules (inherited shape from the AC fork)

- **cli** — clap command surface + bare-invocation normalization.
- **commands** — one handler per command (`setup`, `get`, `current`, `mine`, `search`).
- **controller** — async orchestration: cache read → API fetch → cache write.
- **client** — the Jira Cloud API (REST v3); the only place that speaks Jira.
- **http** — reqwest transport + token host-isolation gate.
- **models** — serde shapes for the Jira issue/search payloads.
- **render** — domain string rendering for human output.
- **agent_json** — pure shaping of the curated `--json` contract.
- **store** — rusqlite persistence: instances, settings, issue cache.
- **i18n** — en + pt-BR display catalog.
