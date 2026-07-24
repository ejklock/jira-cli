---
type: ADR
title: The agent skill is served by a `jira skill` CLI command; per-harness integrations are thin pointers to it
description: A new `jira skill [name]` / `jira skill list` command prints the curated agent skill markdown embedded in the binary from the one canonical .claude/skills/jira/SKILL.md; every other harness (Codex, OpenCode, pi, Copilot, Cursor) integrates via a thin stub that defers to `jira skill jira` instead of copying the contract, and install-skill.sh writes those stubs at each harness's path. Ports ActiveCollab ADR 0057-0059 to jira-cli.
status: Accepted
supersedes:
superseded_by:
tags: [cli, agent, llm, skill, distribution, structure, parity]
timestamp: 2026-07-22T00:00:00Z
---

# 0028. The agent skill is served by a `jira skill` command; per-harness integrations are thin pointers

## Context

[ADR 0004](/adr/0004-agent-json-output-contract.md) gave agents a curated,
minified `--json` read contract across `jira get/current/mine/search`. That
contract had, until now, **no companion agent skill** — nothing documented it in
a form a harness activates on demand. The upstream fork base
(`active-collab-cli`) solved the same problem with an `ac skill` command plus
per-harness thin pointers ([ActiveCollab ADR 0057-0059]); this ADR brings that
design to `jira-cli` as part of the parity program.

Users drive this CLI from several agent harnesses — Claude Code, OpenAI Codex,
OpenCode, pi, GitHub Copilot, Cursor — and each loads agent skills / custom
instructions from a **different** path and, in some cases, a different file
format.

The naive fix — copy a full `SKILL.md` contract into all six locations — creates
the exact drift the living-docs "one home per fact" invariant forbids: a field
rename in the `--json` contract ([ADR 0004](/adr/0004-agent-json-output-contract.md),
locked by `tests/unit/agent_json.rs`) would have to be hand-propagated to six
files, and any missed copy silently teaches an agent a stale schema.

Two facts make a better design possible:

- The binary already **embeds** compile-time assets (`include_str!`), the pattern
  established for the i18n catalog. The single-binary constraint holds
  ([constitution](/constitution.md)).
- Every one of the six harnesses runs its agent with a **shell tool**, and every
  one loads a markdown skill/instruction body the agent treats as instructions.
  So a short body that says *"run `jira skill jira` and follow its output"* is
  actionable in all six — the full contract need not live in the harness file.

## Decision

**The `jira` binary is the single source of truth for the agent skill, exposed
through a new `jira skill` command. Per-harness files are thin pointers to it, not
copies of the contract.**

1. **`jira skill` command (extensible registry).**
   - `jira skill list` — list available skills as `name<TAB>one-line description`.
   - `jira skill <name>` — print that skill's full markdown to stdout (exit 0); an
     unknown name prints an error to stderr and exits `2`.
   - `jira skill` (no argument) — print the single skill's markdown when exactly
     one is registered; otherwise behave as `jira skill list`.
   - `"skill"` joins `KNOWN_COMMANDS` (8 → 9) so a bare `jira skill …` is **not**
     rewritten to `jira get skill` by `normalize_argv`.

2. **One home for the contract body.** The `jira` skill body is
   `include_str!("../.claude/skills/jira/SKILL.md")` — the *same* file Claude Code
   (and, natively, OpenCode and pi) read. The contract text exists in exactly one
   file; the binary embeds it; `jira skill jira` prints it. A schema change edits
   that one file (and `tests/unit/agent_json.rs`), and every consumer — the CLI
   command and every harness pointer — tracks it for free.

3. **Pure, network-free command module.** All shaping lives in a pure
   `src/skill.rs` — a registry `&[SkillEntry { name, description, body }]` and a
   `skill_output(args, &mut impl Write) -> i32` over it, no store/HTTP, unit-tested
   without network (mirroring the `agent_json.rs` purity discipline).

4. **Per-harness integration is a thin pointer.** Every harness other than the
   canonical file receives a small stub whose body defers to `jira skill jira`; it
   carries **no** contract fields, so a `--json` schema change never touches it.
   `install-skill.sh --harness <name>|all` writes the stub at each harness's path:

   | Harness | Path | Format |
   |---|---|---|
   | Claude Code | `.claude/skills/jira/SKILL.md` | full canonical `SKILL.md` (the source; also read natively by OpenCode & pi) |
   | Codex CLI | `.codex/skills/jira/SKILL.md` | thin `SKILL.md` (name+description frontmatter) |
   | OpenCode | `.opencode/skills/jira/SKILL.md` | thin `SKILL.md` |
   | pi | `.pi/skills/jira/SKILL.md` | thin `SKILL.md` |
   | GitHub Copilot | `.github/skills/jira/SKILL.md` | thin `SKILL.md` |
   | Cursor | `.cursor/rules/jira.mdc` | thin MDC rule (`description` set, `alwaysApply: false`) |

   The stub text is single-homed inside `install-skill.sh` (heredoc per format),
   so the pointer wording also has one home. The installer targets the current
   directory for repo-local install and honours a `--dir` override.
   [ADR 0028 §installer], detailed with the `--scope project|global` selector in
   its own follow-up (the parity port of ActiveCollab ADR 0058), lives in the same
   installer.

5. **`--scope project|global` selector.** `install-skill.sh` accepts `--scope
   project` (default; writes under `--dir`, default `.`) and `--scope global`
   (writes each supported harness's real user-level path under `$HOME`):

   | Harness | project (`<dir>/…`) | global (`$HOME/…`) |
   |---|---|---|
   | Claude Code | `.claude/skills/jira/SKILL.md` | `~/.claude/skills/jira/SKILL.md` |
   | pi | `.pi/skills/jira/SKILL.md` | `~/.pi/agent/skills/jira/SKILL.md` |
   | Codex CLI | `.codex/skills/jira/SKILL.md` | `~/.codex/skills/jira/SKILL.md` |
   | OpenCode | `.opencode/skills/jira/SKILL.md` | *(unsupported under global)* |
   | GitHub Copilot | `.github/skills/jira/SKILL.md` | *(unsupported)* |
   | Cursor | `.cursor/rules/jira.mdc` | *(unsupported)* |

   OpenCode, Copilot, and Cursor have no standard user-level skills directory: a
   single named unsupported harness under `--scope global` exits `2`; `--harness
   all --scope global` **skips** the three and installs the three supported,
   exit `0`. `--dir` is incompatible with `--scope global`. When neither `--scope`
   nor `--dir` is given and stdin is a TTY, the installer prompts project-vs-global
   (default project); a non-TTY run (`curl | sh`) silently defaults to project.

## Alternatives considered

- **Copy the full `SKILL.md` into all six harness paths.** Rejected: six copies of
  the `--json` contract is precisely the drift the one-home invariant forbids.
- **Ship only the files, no `jira skill` command.** Rejected: without an on-demand
  command the thin-pointer design has nothing to point at, and scripts/agents in a
  harness we did not anticipate have no way to fetch the contract.
- **A second source file for the embedded body.** Rejected: it re-splits the one
  contract into two homes and denies Claude Code / OpenCode / pi the full skill
  they can read natively for free.
- **Generate each harness file from the canonical `SKILL.md` at install time (full
  copy).** Rejected: still N materialized copies to re-generate on every change.

## Consequences

**Positive:**

- The `--json` contract gains its first agent skill, and it has one home; the CLI
  command and all six harness pointers cannot drift from it.
- `jira skill` is a general, testable read surface: any agent or script — in any
  harness, or none — can fetch the contract on demand.
- Adding a second skill later is a new registry entry (+`include_str!`), not a new
  command; `jira skill list` already accommodates it.
- `src/skill.rs` is pure and unit-tested; the registry shape is locked by tests.

**Accepted trade-offs:**

- `include_str!` binds the skill body at **compile time**: editing the contract
  needs a rebuild (acceptable — we ship a binary).
- Harnesses that cannot read `.claude/skills/` natively (Codex, Copilot, Cursor)
  need one `install-skill.sh` run to place their pointer; documented in the README.
- The thin pointer adds one indirection (the agent runs `jira skill jira` before
  it has the contract). This is the deliberate cost that buys drift-freedom.

## Related

- ADR: [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md) — the `--json` contract this skill documents.
- Constitution: [/constitution.md](/constitution.md) — the single-binary + embedded-asset constraints.
- BDR: [/bdr/0019-jira-skill-command-behaviors.md](/bdr/0019-jira-skill-command-behaviors.md) — the observable `jira skill` + installer behavior + test matrix.
- Upstream parity: ActiveCollab ADR 0057 (skill command), 0058 (`--scope`), 0059 (name after the product) — ported here as one ADR.
