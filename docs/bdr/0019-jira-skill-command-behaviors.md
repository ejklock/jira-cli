---
type: BDR
title: "`jira skill` serves the embedded agent skill; the installer writes thin per-harness pointers with a project|global scope"
description: "`jira skill list` prints each registered skill as name<TAB>first-sentence-description; `jira skill jira` prints the full embedded SKILL.md and exits 0; `jira skill <unknown>` errors to stderr and exits 2; bare `jira skill` prints the single registered skill (never launches the TUI, never hits the network). install-skill.sh writes a thin `jira skill jira` pointer per harness under --scope project (default, under --dir) or --scope global (user-level paths for claude/pi/codex; opencode/copilot/cursor unsupported); --dir is incompatible with global; a TTY with neither flag prompts, a non-TTY defaults to project."
status: Accepted
superseded_by:
supersedes:
tags: [cli, agent, skill, distribution, installer, parity]
timestamp: 2026-07-22T00:00:00Z
---

# 0019. `jira skill` command + install-skill.sh behaviors

## Context

The parity port of the ActiveCollab agent-skill distribution to `jira-cli`
([ADR 0028](/adr/0028-agent-skill-served-by-jira-skill-command.md)). The binary
becomes the single source of truth for the agent skill via a new `jira skill`
command, and `install-skill.sh` writes thin per-harness pointers to it. This BDR
pins the observable behavior of both surfaces.

## Textual Description

### `jira skill` command (pure, no network, never launches the TUI)

- **`jira skill list`** prints one line per registered skill: the skill `name`, a
  TAB, then the first sentence of its description. Exit `0`.
- **`jira skill jira`** prints the full embedded canonical `SKILL.md` body to
  stdout, byte-for-byte the embedded file. Exit `0`.
- **`jira skill <unknown>`** prints a clear "unknown skill: <name>" error to
  **stderr** (plain English — the skill surface is an agent-facing English
  contract, not localized) and exits `2`; stdout stays empty.
- **`jira skill`** (no argument) prints the single registered skill's full body
  (there is exactly one, `jira`). If more than one were ever registered it would
  behave as `jira skill list`.
- The command is **pure**: it never opens the store, never hits the network, and
  never launches the browse TUI even on a terminal. `"skill"` is in
  `KNOWN_COMMANDS`, so `jira skill …` is not rewritten to `jira get skill`.

### `install-skill.sh` (thin pointers, project|global scope)

- **`--harness <name>`** writes the thin pointer for one harness; **`--harness
  all`** writes every harness supported under the chosen scope.
- **`--scope project`** (default) writes under `--dir` (default `.`); **`--scope
  global`** writes each supported harness's user-level path under `$HOME`.
- **Unsupported under global:** opencode, copilot, cursor have no standard
  user-level skills dir. A single named one under `--scope global` exits `2`;
  `--harness all --scope global` skips them, installs claude/pi/codex, exits `0`.
- **`--dir` + `--scope global`** is contradictory → exits `2`.
- **`--force`** overwrites an existing target; without it an existing file is left
  untouched and reported "exists, skipping".
- **Scope prompt:** neither `--scope` nor `--dir` given **and** stdin is a TTY →
  prompt project/global (default project). Non-TTY (`curl | sh`) → project, no
  prompt.
- Each written stub carries **no** `--json` schema fields — only the instruction
  to run `jira skill jira` — so it can never drift from the contract.

## Scenarios

- **S1 — list.** `jira skill list` → `jira<TAB><first sentence>`, exit 0.
- **S2 — print by name.** `jira skill jira` → the full embedded SKILL.md body on
  stdout, exit 0.
- **S3 — bare.** `jira skill` (single registered skill) → the same full body as
  S2, exit 0.
- **S4 — unknown.** `jira skill nope` → localized error on stderr, empty stdout,
  exit 2.
- **S5 — no rewrite.** `normalize_argv(["skill"])` is unchanged (not rewritten to
  `["get","skill"]`); `"skill"` is a known command.
- **S6 — install project.** `install-skill.sh --harness claude --dir <tmp>` writes
  `<tmp>/.claude/skills/jira/SKILL.md` containing the `jira skill jira` pointer.
- **S7 — install all project.** `--harness all --dir <tmp>` writes all six
  harness paths (SKILL.md for five, `.cursor/rules/jira.mdc` for cursor).
- **S8 — global supported.** `--harness pi --scope global` writes
  `~/.pi/agent/skills/jira/SKILL.md` (user-level path, not `.pi/skills/…`).
- **S9 — global unsupported (named).** `--harness cursor --scope global` prints
  the unsupported message to stderr, exit 2.
- **S10 — global all skips unsupported.** `--harness all --scope global` installs
  claude/pi/codex, skips opencode/copilot/cursor, exit 0.
- **S11 — dir+global conflict.** `--dir X --scope global` → error, exit 2.
- **S12 — force.** Re-running without `--force` reports "exists, skipping"; with
  `--force` overwrites.

## Test Matrix

| Scenario | Trigger | Expected | Verify |
|---|---|---|---|
| S1 | `jira skill list` | `jira\t<first sentence>`, exit 0 | unit (skill_output over a Vec<u8>) |
| S2 | `jira skill jira` | full embedded SKILL.md, exit 0 | unit — byte-equality vs include_str! |
| S3 | `jira skill` | same body as S2, exit 0 | unit |
| S4 | `jira skill nope` | stderr error, empty stdout, exit 2 | unit |
| S5 | `normalize_argv(["skill",...])` | unchanged (no get-prefix) | unit (cli.rs) |
| S6 | installer, one harness, project | correct path written, pointer body | bats/shell integration test |
| S7 | installer, all, project | six paths written | shell integration test |
| S8 | installer, pi, global | `~/.pi/agent/skills/jira/SKILL.md` written | shell integration test (HOME=tmp) |
| S9 | installer, cursor, global | stderr message, exit 2 | shell integration test |
| S10 | installer, all, global | claude/pi/codex written, three skipped, exit 0 | shell integration test (HOME=tmp) |
| S11 | installer, --dir + --scope global | error, exit 2 | shell integration test |
| S12 | installer re-run ±--force | skip vs overwrite | shell integration test |

## Related

- ADR: [/adr/0028-agent-skill-served-by-jira-skill-command.md](/adr/0028-agent-skill-served-by-jira-skill-command.md)
- ADR: [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md) — the `--json` contract the skill documents.
- Skill: [.claude/skills/jira/SKILL.md](../../.claude/skills/jira/SKILL.md) — the canonical embedded body.
