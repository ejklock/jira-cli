---
type: PRD
title: "Total parity with active-collab-cli — design, interactions, comment writes"
description: Bring jira-cli to full feature parity with its fork base active-collab-cli — the vibrant-dashboard visual design, CLI/UX behaviors (bare-TTY default, 401 re-auth, SWR first paint), mouse/selection/inline-link/attachment/projects interactions, and comment writes — adapted to Jira Cloud specifics.
status: Accepted
supersedes: "PRD 0002 §Non-goals (projects axis, attachments panel, mouse) — reopened here"
superseded_by:
tags: [parity, tui, design, mouse, attachments, comments, write]
timestamp: 2026-07-06T00:00:00Z
---

# 0003. Total parity with active-collab-cli

## Problem

jira-cli forked `active-collab-cli` ([ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md))
but shipped only the read core + a basic browse TUI. The fork base has since
matured far beyond that: a designed visual system (vibrant dashboard, task
cards, stacked detail panels — its ADR 0009/0026), mouse + text-selection +
inline-link + attachment interactions, resilience behaviors (401 re-auth, SWR
first paint, bare-TTY default), and comment writes. The operator now uses both
tools daily and the experience gap is the friction. Decision (user,
2026-07-06): **total parity, including design**, adapted to Jira's specifics.

## Scope decisions this PRD records

- **Reopens** the PRD 0002 non-goals *Projects browse axis*, *assets/attachments
  panel*, and *mouse-first interaction*: they DO map onto Jira (project field,
  `attachment` field, same terminal). PRD 0002 stays accepted for what it
  shipped; its non-goals table is superseded by this section.
- **Comment writes** enter scope via Constitution **Amendment 1** and
  [ADR 0015](/adr/0015-comment-write-enablement.md). All other writes stay out.
- **Deliberately NOT ported** (AC-specific mechanics whose problem does not
  exist in Jira): the project-name directory cache (AC ADR 0014 — Jira embeds
  project data in the issue payload), the `user_map` identity fetch (Basic auth
  already knows the email), and the AC internal refactor history (HT/ARCH
  slices — we port their END state inside the slices below).

## Requirements

**Group D — visual design parity** ([ADR 0014](/adr/0014-tui-visual-design-system.md)):

- **R-D1** — truecolor sober cool-retro palette in a central `theme.rs`; a
  logged-in identity header bar (`email · instance`, `(+N more)` when
  multi-instance); themed footer.
- **R-D2** — the browse list renders per-issue bordered cards: line 1
  `KEY summary`, line 2 a relative color-coded due date (overdue red, near
  amber) `· status · project`; whole selected card highlighted.
- **R-D3** — the detail screen is stacked rounded panels (Details meta panel
  with a Title row, Description, Comments) in a single global scroll with a
  visible scrollbar; the issue summary is promoted to the frame border title.
- **R-D4** — a contextual (mode-aware) footer plus a thin transient status
  line.
- **R-D5** — ADF `table` nodes render legibly in detail/comments (strike,
  underline, codeBlock already covered).

**Group E — behavior parity:**

- **R-E1** — a bare `jira` invocation in a TTY defaults to `mine`; non-TTY
  keeps the help/error contract (AC ADR 0013).
- **R-E2** — HTTP 401 anywhere (CLI get/current/mine/search, TUI fetch) yields
  an actionable re-auth message pointing at `jira setup` and a non-zero exit
  (CLI) / thin status line (TUI), never a raw error dump (AC RA1–RA3).
- **R-E3** — browse/mine first-paint from the local cache, then refresh in the
  background and repaint (SWR), with single-flight refresh (AC S8/BDR 0005).

**Group B — interaction parity** (per-slice ADRs at execution time):

- **R-B1** — mouse: click selects/activates list rows and detail links; wheel
  scrolls list and detail.
- **R-B2** — body links render inline as text + visible URL, activatable from
  the visible region (keyboard nav from A2 kept), Ctrl/Cmd+click opens.
- **R-B3** — app-managed text selection: drag highlights, releases copy to the
  clipboard with feedback.
- **R-B4** — an attachments panel in detail: labels derived (filename →
  anchor text → host), Ctrl/Cmd+click opens.
- **R-B5** — a Projects browse axis: a projects list screen drilling into
  per-project issues (screen stack, Esc pops).

**Group C — comment writes** ([ADR 0015](/adr/0015-comment-write-enablement.md)):

- **R-C1** — compose a new comment in the TUI (modal overlay, multi-line),
  `POST`, server-truth refresh.
- **R-C2** — edit/delete YOUR OWN comment via permission-aware affordances
  (`PUT`/`DELETE`), delete behind a Sim/Não confirm modal.
- **R-C3** — a non-TTY `jira comment <KEY> -m/stdin` command with `--json`
  result.

## Acceptance

Parity is accepted when, against a real Jira Cloud instance, every requirement
above is demoable and the corresponding AC behavior (per the linked AC
issue/BDR, adapted) is observable in jira-cli. Execution order: **D → E → B →
C**; every slice keeps the constitution non-negotiables (token host isolation
extended to write endpoints, local-first reads, JSON/text no-drift, pure core).

## References

- Constitution: [/constitution.md](/constitution.md) (Amendment 1)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md)
- ADR: [/adr/0014-tui-visual-design-system.md](/adr/0014-tui-visual-design-system.md)
- ADR: [/adr/0015-comment-write-enablement.md](/adr/0015-comment-write-enablement.md)
- Fork-base trail: `active-collab-cli` docs (ADR 0009, 0013, 0026; BDR 0001, 0004, 0005, 0020; issues V5/V6/D1/D2/N1/N2/M1/M2/RA1–3/S8/C1–C3)
