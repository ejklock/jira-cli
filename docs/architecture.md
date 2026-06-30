---
type: Architecture View
title: Architecture — module structure and read data flow
description: Living Mermaid views of the jira-cli Rust module structure and the local-first read data flow.
tags: [architecture, rust, jira]
timestamp: 2026-06-29T00:00:00Z
---

# Architecture

Living diagrams of the `jira-cli` Rust app, a fork of `active-collab-cli`
([ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md)) with the domain layer
swapped to Jira Cloud ([ADR 0002](/adr/0002-jira-cloud-only-basic-auth.md)). Node
names use [context-index](/context/index.md) vocabulary. v1 is CLI-first. The fork's
`tui/` was **not** carried over; Phase 2 adds a fresh, read-only `browse` TUI built as
an Elm/TEA shell on ratatui + crossterm over the same domain core
([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)) — the B0 skeleton (command +
raw-mode shell) has landed. This view is updated as each structural change lands
(maintenance invariant).

## Design principles

The architecture is a **modular monolith** (single binary, no service/process
split — microservices are earned, not a starting point) built from **deep modules**
(Ousterhout: powerful functionality behind a narrow interface) over a **Functional
Core, Imperative Shell** (Bernhardt) — the constitution's "pure, testable core"
non-negotiable. Pure logic (ref parsing, branch-key extraction, JQL build,
`agent_json` shaping, ADF flatten) touches no I/O; the thin shell (`controller`
async, `store`, the gouqi-backed `client`) does the side effects.

Three standing guardrails keep modules deep and the system intelligible. They are
**constraint ACs on every slice** (Reviewer-judged):

1. **Curated domain models, not a gouqi mirror.** `models` exposes only the fields
   the tool uses; it is **not** a 1:1 pass-through of gouqi's `rep` types. A
   pass-through mapping adds interface cost while hiding nothing (a shallow module)
   — the point of the boundary is to decouple our contract from gouqi, not to copy it.
2. **Thin commands.** `commands` handlers dispatch and format only; all
   orchestration lives in `controller`/`client`. Logic scattered across command
   handlers is "classitis" and defeats traceability.
3. **One seam, earned.** The `JiraClient` trait is the single deliberate abstraction
   seam (it pays for itself: swap path + host-isolation + testability). We do not
   wrap every module behind a trait "for testability" (test-induced design damage)
   — prefer a pure function before reaching for DI; add a seam only where a deletion
   test shows it earns its keep.

## Module structure

```mermaid
flowchart TD
    main["shell (main.rs)\ntokio lifecycle + dispatch"] --> cli["cli (clap)\n+ bare-invocation normalize"]
    cli --> commands["commands\nsetup · get · current · mine · search"]
    commands --> controller["controller\n(async: cache → fetch → cache)"]
    controller --> client["client (JiraClient trait)\ngouqi-backed impl\n(SearchApiVersion::V3 · Basic auth)\nhost-pinned construction"]
    client --> gouqi["gouqi crate\nreqwest + rustls\n/rest/api/3/issue · /search/jql · /myself"]
    client --> models["models (serde)\nIssue · SearchResult · Comment\n(mapped from gouqi rep)"]
    controller --> store["store\n(rusqlite: instances · settings ·\nissue cache keyed by (instance, key))"]
    commands --> render["render\ndomain string rendering (human)\n+ ADF → plain text"]
    commands --> agent_json["agent_json\npure --json shaping"]
    commands --> i18n["i18n (en · pt-BR)"]
    cli --> tui["tui (browse, Phase 2)\nread-only Elm/TEA shell\n(ratatui + crossterm)"]
    tui --> client
    tui --> store
    tui --> render
    tui --> i18n
```

The `tui` (`browse`) shell is a **second imperative shell** over the same functional
core: a pure `update(Model, Msg) -> (Model, Vec<Cmd>)` drives navigation/scroll/search
state (unit + ratatui `TestBackend` tested), while `Cmd` effects reuse the existing
data seams — `client` (`JiraClient::search` for lists, cache-or-fetch for detail) and
`store` — never the rendering `*_core` functions. Read-only by construction
([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)).

**Boundaries / fitness:**

- **`client` (the gouqi-backed `JiraClient` impl) is the only outbound-network
  boundary** ([ADR 0005](/adr/0005-jira-client-on-gouqi-behind-trait.md)). gouqi
  owns the reqwest transport, so token host isolation is a **wrapper invariant**:
  the wrapper is the single `gouqi::Jira` construction site and always builds it
  from the resolved instance host. Gate-checked by a unit test
  ([PRD 0001](/prd/0001-jira-cloud-read-cli.md) NFR-1) that the wrapper never
  targets a host other than the instance `base_url` host.
- **`store` owns all persistence;** no other module opens the SQLite file. The
  issue cache is keyed by `(instance_name, issue_key)`
  ([ADR 0003](/adr/0003-issue-identity-and-cache-key.md)).
- **`agent_json` is pure** and derived from the same helpers as `render`, so the
  JSON contract and the human text never drift
  ([ADR 0004](/adr/0004-agent-json-output-contract.md)); a field rename/drop fails
  a unit test.
- **`client` is a clean trait seam** so a Phase-2 Jira Server/DC (REST v2) adapter
  is additive, not a rewrite.

## Read data flow (local-first)

```mermaid
sequenceDiagram
    actor User
    participant CLI as cli/commands
    participant Ctl as controller
    participant Cache as store (cache)
    participant API as client (gouqi)
    participant Jira as Jira Cloud

    User->>CLI: jira get PROJ-123 [--refresh]
    CLI->>Ctl: resolve instance + key
    Ctl->>Cache: read cached (instance, key)?
    alt cache hit and not --refresh
        Cache-->>Ctl: cached issue
    else miss or --refresh
        Ctl->>API: GET /rest/api/3/issue/PROJ-123 (Basic auth, host-gated)
        API->>Jira: request
        Jira-->>API: issue payload
        API-->>Ctl: Issue
        Ctl->>Cache: write (instance, key)
    end
    Ctl-->>CLI: Issue
    CLI-->>User: human render | --json
```

The cache makes reads **local-first**: a cached `get` succeeds with the network
disabled (NFR-2); `--refresh` forces the fetch arm. The list commands
(`mine`/`search`) issue a JQL request to `/rest/api/3/search/jql` and render the
result page; per-issue caching of list results is deferred (PRD open question).

## Quality gates

The crate inherits the AC `comment_policy` integration test (`tests/comment_policy.rs`,
run by `cargo test`): no banner/divider comments and no commented-out Rust code,
while doc comments (`///`, `//!`) and non-obvious why-comments are allowed.
