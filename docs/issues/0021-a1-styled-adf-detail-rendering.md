---
type: Issue
title: "A1 — styled ADF rendering in the browse TUI detail description"
description: Render the browse TUI detail's ADF description with inline styling (bold/italic/code/strike/underline/link) instead of flattening to plain text. Introduce a pure ratatui-free rich model (RichStyle/RichSpan/RichLine) + an adf_to_rich walker in src/render.rs; view_detail maps it to ratatui spans. adf_to_plain_text stays byte-stable for the CLI get render + agent_json. First read-only slice of the active-collab-cli parity program (Group A).
status: done
tracker:
tags: [tui, browse, render, adf, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# A1 — styled ADF rendering in the browse TUI detail

## Objective link

[PRD 0002](/prd/0002-interactive-browse-tui.md) R2 (open detail), realizing
[ADR 0010](/adr/0010-styled-adf-rendering-browse-tui-detail.md) over the TUI arch of
[ADR 0007](/adr/0007-browse-tui-elm-architecture.md). Part of the AC-cli **total-parity**
program (Group A — read-only): AC renders styled rich text (`src/richtext.rs`); jira-cli
only flattens ADF to plain text. First parity slice; the `adf_to_rich` primitive is reused
by A2 (clickable links) and A4 (comment display/nav).

## Context manifest

- **Read first:** `src/render.rs` — `adf_to_plain_text` (L14) and its private walker
  (`flatten_node`/`flatten_inline_node` L35–140+): the ADF node/mark structure to mirror.
  Inline `text` nodes carry `marks: [{ type, attrs? }]`; the plain walker ignores them.
- `src/tui/view.rs` — `view_detail` (L145+) composes key/summary/status/type/assignee +
  `crate::render::adf_to_plain_text(description)` into ONE `String` and renders it as a
  single scrollable `Paragraph` (`.scroll((model.detail_scroll, 0))`). This is where the
  description portion becomes a styled `Text`.
- `src/models.rs` — `Issue.description: Option<String>` (raw ADF JSON string).
- `tests/unit/render.rs` — the `adf_to_plain_text_*` tests LOCK the plain output; they must
  stay green (byte-stable). Add new `adf_to_rich_*` tests alongside.

## Approach (decided — see ADR 0010)

- In `src/render.rs`, add the neutral rich model (no ratatui):
  `RichStyle { bold, italic, code, strike, underline, link: Option<String> }` (derive
  Debug, Clone, PartialEq, Eq, Default), `RichSpan { text: String, style: RichStyle }`,
  `type RichLine = Vec<RichSpan>`.
- Add `pub fn adf_to_rich(raw: &str) -> Vec<RichLine>`: walk the same ADF block structure
  as `adf_to_plain_text` (paragraph/heading/bulletList/orderedList/listItem/codeBlock/
  blockquote/panel/rule/hardBreak) but carry inline marks into `RichStyle` per `text` run:
  `strong`→bold, `em`→italic, `code`→code, `strike`→strike, `underline`→underline,
  `link`→`link = Some(attrs.href)` (+ underline). Non-ADF / non-`doc` input → a single
  unstyled line with the raw string. Block shaping (list markers `1. `/`- `, indentation,
  code blocks) mirrors the plain walker so both look consistent.
- Do NOT modify `adf_to_plain_text` (keep it byte-stable for CLI/agent_json). A shared
  private traversal may be factored ONLY if it leaves the plain output byte-identical;
  otherwise keep two focused walkers.
- In `src/tui/view.rs` `view_detail`, render the description via `adf_to_rich` mapped to
  ratatui `Text`/`Line`/`Span`: bold→`Modifier::BOLD`, italic→`ITALIC`, strike→
  `CROSSED_OUT`, underline→`UNDERLINED`, code→a dim/distinct `Style`, link→`UNDERLINED`
  (href retained in the model, NOT yet clickable — that is A2). Keep the metadata lines
  (labels via `t()`) as today; keep `Paragraph::scroll` over the styled `Text`. The
  `Description:` label stays translated.
- Read-only; no new key bindings; comments are NOT added to the detail here (that is A4).

## Vertical Demo

- **Given** `jira browse` and an issue whose description has bold, italic, inline code and
  a link,
- **When** I open its detail,
- **Then** the bold text renders bold, italic italic, code in the code style, and the link
  underlined — no raw ADF/markup leaking — and ↑/↓ still scrolls the styled body.
- **And** `jira get KEY` plain output + `agent_json` are byte-identical to before (the
  plain walker is untouched).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | `adf_to_rich` maps each inline mark to the right `RichStyle` flag (strong→bold, em→italic, code→code, strike→strike, underline→underline, link→`link=Some(href)`), and composes multiple marks on one run; block structure (paragraphs, lists, code blocks, hardBreak) matches the plain walker's shaping | test (unit on `adf_to_rich`) |
| AC2 | behavior | `view_detail` renders a bold description run with `Modifier::BOLD` and a link run with `UNDERLINED` (styled `Text`), asserted via ratatui `TestBackend` (cell style, not just text) | test (TestBackend) |
| AC3 | constraint | `adf_to_plain_text` output is byte-identical to before for every existing `adf_to_plain_text_*` test (the plain seam for CLI `get` + `agent_json` is untouched) | test (existing render tests stay green) |
| AC4 | constraint | The rich model + `adf_to_rich` are pure and ratatui-free (live in `src/render.rs`, no ratatui import); the ratatui mapping lives only in `view.rs` (NFR-B1 pure core / Humble Object) | inspection |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; cyclomatic ≤10 (≤8 new fns) / cognitive within ceiling; a mutant that drops a mark→style mapping is killed by AC1; no superfluous comments/banners/commented-out code | command (clippy + fmt + comment_policy + complexity) |

## Out of scope

- Clickable links / opening the href (A2 — the href is only retained here).
- Displaying or navigating comments in the TUI detail (A4).
- Styling the CLI `get` output (stays plain text); any agent_json change.
- Groups B (Projects axis / assets panel / mouse) and C (comment writes) — parked behind
  their own ADRs / a constitution amendment.

## blocked_by

(none — foundational read-only parity slice; A2 and A4 build on it)
