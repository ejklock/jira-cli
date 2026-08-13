---
type: ADR
title: Rename the agent skill identifier from `jira` to `jira-ticket`
description: The embedded skill registry entry, the canonical .claude/skills path, every per-harness stub path, and the pointer command change from the name `jira` to `jira-ticket`. The architecture of ADR 0028 (binary as SSOT, thin pointers) is unchanged. No back-compat alias is kept. The canonical SKILL.md also gains a sandbox note that documents the JIRA_DB override for read-only config directories.
status: Accepted
supersedes:
superseded_by:
tags: [cli, agent, llm, skill, distribution, naming]
timestamp: 2026-08-13T00:00:00Z
---

# 0030. Rename the agent skill identifier from `jira` to `jira-ticket`

## Context

[ADR 0028](/adr/0028-agent-skill-served-by-jira-skill-command.md) named the
embedded agent skill `jira`. That name collides with the binary name and with
generic "jira" tooling that agents already know. A field failure showed the
cost: a sandboxed agent looked for a skill named `jira-ticket`, found nothing,
and guessed a non-existent script path instead of reading the installed
`SKILL.md`. The skill name must match what agents look for and must not be
ambiguous with the `jira` binary itself.

The same failure exposed a documentation gap. In a sandbox, the default DB path
(`~/.config/jira/jira.db`) can sit on a read-only filesystem. The CLI supports
the `JIRA_DB` environment override, but no skill text documented it, so the
agent had to discover the workaround by trial and error.

## Decision

**The skill identifier becomes `jira-ticket` everywhere. The ADR 0028
architecture — binary as single source of truth, per-harness thin pointers —
is unchanged.**

1. **Registry.** The single `SkillEntry` in `src/skill.rs` is renamed to
   `jira-ticket`. `jira skill jira-ticket` prints the full body;
   `jira skill list` prints `jira-ticket<TAB>description`; the bare
   `jira skill` still prints the single registered skill.
2. **Canonical file.** The embedded source moves to
   `.claude/skills/jira-ticket/SKILL.md` (frontmatter `name: jira-ticket`).
3. **Installer.** `install-skill.sh` writes every stub under
   `skills/jira-ticket/` (cursor: `.cursor/rules/jira-ticket.mdc`) and the
   stub bodies point to `jira skill jira-ticket`.
4. **No alias.** `jira skill jira` now exits `2` (unknown skill). Stubs and
   binary ship together; existing installs re-run
   `install-skill.sh --force` after upgrade. An alias would be a second name
   for one skill and would keep the ambiguity this rename removes.
5. **Sandbox note.** The canonical `SKILL.md` gains a short section that
   documents the `JIRA_DB` override and the copy-to-writable-path workaround
   for read-only config directories.

## Consequences

- Agents that search for a ticket-shaped skill find `jira-ticket` by name;
  the binary name `jira` no longer doubles as a skill identifier.
- Old stubs that say `jira skill jira` break loudly (exit 2) instead of
  silently drifting; the fix is one installer re-run.
- Historical docs (ADR 0028, issues 0058/0059) keep the old name — they
  record the state at their timestamp. Living docs (BDR 0019, architecture,
  README, CHANGELOG) are updated to the new name.
