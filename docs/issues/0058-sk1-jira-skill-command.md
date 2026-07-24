---
type: Issue
title: "SK1 — `jira skill` command: pure src/skill.rs registry + skill_output, include_str! the canonical SKILL.md, 'skill' joins KNOWN_COMMANDS"
description: Add the on-demand agent-skill read command. A pure src/skill.rs holds a registry &[SkillEntry { name, description, body }] with one entry (name "jira", body include_str!("../.claude/skills/jira/SKILL.md")) and skill_output(args, &mut impl Write) -> i32. `jira skill list` prints name<TAB>first-sentence; `jira skill jira` prints the full body (exit 0); `jira skill` with one entry prints that body; an unknown name errors to stderr and exits 2. "skill" joins KNOWN_COMMANDS (8->9) so normalize_argv does not rewrite it; a Command::Skill variant dispatches to skill_output in main.rs. Unit-tested without network (byte-equality vs include_str!, list format, unknown->2, no-arg single). No store, no HTTP, never launches the TUI.
status: open
labels: [cli, agent, skill, distribution, parity]
blocked_by:
tracker:
timestamp: 2026-07-22T00:00:00Z
---

## SK1 — `jira skill` command

Implements [ADR 0028](/adr/0028-agent-skill-served-by-jira-skill-command.md) §1–§3
and [BDR 0019](/bdr/0019-jira-skill-command-behaviors.md) S1–S5 (the command half).
Foundation slice — no installer yet (that is SK2). Depends on the canonical
`.claude/skills/jira/SKILL.md` authored in the A1 docs slice.

Scope: `src/skill.rs` (new), `src/cli.rs`, `src/main.rs`, `tests/unit/skill.rs` (new).

- **Pure module** `src/skill.rs` mirroring the `agent_json.rs` purity discipline:
  a `SkillEntry { name: &str, description: &str, body: &str }`, a `const REGISTRY:
  &[SkillEntry]` with one entry — `name: "jira"`, `body:
  include_str!("../.claude/skills/jira/SKILL.md")`, `description` = the first
  sentence of the skill's frontmatter description — and
  `skill_output(args: &[String], out: &mut impl Write) -> i32`.
- **Behavior** (BDR 0019): `list` → one `name\t<first sentence>` line per entry,
  exit 0; `<name>` → full body, exit 0; unknown `<name>` → a clear plain
  (non-i18n) error on **stderr**, empty stdout, exit 2; no argument with a single
  registered entry →
  that entry's full body, exit 0.
- **Wiring**: add `"skill"` to `KNOWN_COMMANDS` in `src/cli.rs` (array length
  8 → 9) so `normalize_argv` does not rewrite `jira skill …` to `jira get skill`;
  add a `Command::Skill` clap variant and dispatch it to `skill_output` in
  `src/main.rs`. The command is pure — it opens no store, makes no request, and
  never launches the browse TUI.
- **Tests** (`tests/unit/skill.rs`, `#[path]`-included like the other unit
  modules): byte-equality of `jira skill jira` output vs `include_str!`; the
  `list` line format; unknown → exit 2 + stderr; no-arg single → the body; and a
  `normalize_argv(["skill", …])` no-rewrite assertion in the cli.rs tests.
- Update `docs/architecture.md` (+ its Mermaid module diagram) to add the new
  `skill` command / `src/skill.rs` module — living-docs maintenance rule.
