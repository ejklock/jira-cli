---
type: Issue
title: "SK2 — install-skill.sh: thin per-harness pointers to `jira skill jira` with --harness <name>|all and --scope project|global; fix install.sh fork bug"
description: Add install-skill.sh that writes a thin `jira skill jira` pointer stub per agent harness (claude/codex/opencode/pi/copilot/cursor), single-homed in heredocs, carrying no --json schema fields. --harness <name>|all selects harnesses; --scope project (default, under --dir, default .) vs --scope global (user-level $HOME paths for claude/pi/codex only; opencode/copilot/cursor unsupported-under-global). --dir is incompatible with --scope global (exit 2); a named unsupported harness under global exits 2, `all` under global skips them and exits 0; --force overwrites; a TTY with neither --scope nor --dir prompts (default project), a non-TTY defaults to project. ALSO fix the fork bug in install.sh (REPO ejklock/active-collab-cli -> the jira-cli repo, BIN_NAME active-collab -> jira). Shell integration tests cover the path map, scope policy, conflict, and force.
status: done
labels: [cli, agent, skill, installer, distribution, parity, bugfix]
blocked_by: [0058]
tracker:
timestamp: 2026-07-22T00:00:00Z
---

## SK2 — install-skill.sh installer + install.sh fork-bug fix

Implements [ADR 0028](/adr/0028-agent-skill-served-by-jira-skill-command.md) §4–§5
and [BDR 0019](/bdr/0019-jira-skill-command-behaviors.md) S6–S12 (the installer
half). Ports the ActiveCollab `install-skill.sh` (ADR 0057/0058/0059), renamed for
`jira`. Blocked by SK1 (the pointer targets `jira skill jira`, which SK1 adds).

Scope: `install-skill.sh` (new), `install.sh` (bug fix), `tests/` shell integration.

- **Thin pointer stubs, single-homed in heredocs** — a `SKILL.md` body (frontmatter
  `name: jira` + description; body says "run `jira skill jira` and follow its
  output") and a `.mdc` body for Cursor. No `--json` schema fields, so a contract
  change never touches them.
- **`--harness <name>|all`** over the path map (project | global):

  | Harness | project (`<dir>/…`) | global (`$HOME/…`) |
  |---|---|---|
  | claude | `.claude/skills/jira/SKILL.md` | `~/.claude/skills/jira/SKILL.md` |
  | pi | `.pi/skills/jira/SKILL.md` | `~/.pi/agent/skills/jira/SKILL.md` |
  | codex | `.codex/skills/jira/SKILL.md` | `~/.codex/skills/jira/SKILL.md` |
  | opencode | `.opencode/skills/jira/SKILL.md` | *(unsupported)* |
  | copilot | `.github/skills/jira/SKILL.md` | *(unsupported)* |
  | cursor | `.cursor/rules/jira.mdc` | *(unsupported)* |

- **`--scope` policy** (BDR 0019 S8–S11): global supports claude/pi/codex; a named
  opencode/copilot/cursor under global prints the unsupported message to stderr and
  exits 2; `--harness all --scope global` skips the three and exits 0; `--dir` +
  `--scope global` → error exit 2; `--force` overwrites, else "exists, skipping".
- **Scope prompt**: neither `--scope` nor `--dir` and a TTY → prompt (default
  project); non-TTY (`curl | sh`) → project, no prompt.
- **install.sh fork-bug fix** (separate defect surfaced during this epic): the
  release installer still points at the fork base — `REPO="ejklock/active-collab-cli"`
  and `BIN_NAME="active-collab"`. Repoint `REPO` to the jira-cli GitHub repo and
  `BIN_NAME` to `jira` so `curl … | sh` installs the right binary and names it
  `jira`. Keep the OS/arch asset logic unchanged.
- **Tests**: shell integration (a `tests/` bats-or-sh harness run under Docker or
  host `sh`) asserting the written path per harness/scope, the unsupported-under-
  global exits, the `--dir`+global conflict, and the `--force` skip/overwrite.
- Update `README` (the install-skill one-liner + per-harness note) and
  `docs/architecture.md` if the distribution surface is diagrammed.
