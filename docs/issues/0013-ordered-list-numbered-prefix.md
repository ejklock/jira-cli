---
type: Issue
title: "ADF orderedList renders a numbered prefix (latent debt fix)"
description: The ADF flattener threads an `ordered` flag into flatten_list_item but ignores it (_ordered), so orderedList items render with a '- ' bullet like bulletList. Make orderedList items render a 1-based numbered prefix (1. 2. 3.), bulletList unchanged.
status: done
tracker:
tags: [render, adf, debt]
timestamp: 2026-06-30T00:00:00Z
---

# ADF orderedList renders a numbered prefix

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) (human issue rendering — `get`/`current` show the
description). The ADF flattener doc comment (render.rs:11) already claims it "Handles … orderedList …",
but the `ordered` flag is dead: every list renders as a `- ` bullet. This corrects the latent bug so
ordered lists are actually ordered.

## Context manifest

- **Read first:** `src/render.rs` — `flatten_list` (L60, passes `ordered` to each item),
  `flatten_list_item` (L72, signature carries `_ordered` and IGNORES it), `flatten_list_item_child`
  (L79, hardcodes the `"{indent}- "` marker for the first paragraph at L89). `flatten_node` routes
  `orderedList` → `flatten_list(.., true)` (L44) and `bulletList` → `flatten_list(.., false)` (L43).
- **The dead flag:** `flatten_list` already knows `ordered`; the index is available via `enumerate`.
  The marker just never reaches the prefix.

## Approach (decided)

- `flatten_list` (render.rs:60): `enumerate()` the items; compute a per-item **marker** string —
  `format!("{}. ", i + 1)` when `ordered` (1-based), else `"- ".to_string()` — and pass the marker
  (a `&str`) to `flatten_list_item`. Drop the `ordered` bool from the item fns (replaced by the
  marker).
- `flatten_list_item` (render.rs:72): accept `marker: &str` instead of `_ordered`; pass it to
  `flatten_list_item_child` for the first paragraph.
- `flatten_list_item_child` (render.rs:79): replace the hardcoded `out.push_str(&format!("{indent}- "))`
  (L89) with `out.push_str(&format!("{indent}{marker}"))`. Everything else (nested lists, non-first
  paragraphs, indent depth) unchanged.
- **Nested numbering resets per level automatically:** a nested `orderedList` recurses through
  `flatten_node` → `flatten_list`, which re-`enumerate`s, so each level numbers independently.

## Vertical Demo

- **Given** an issue whose description is an ADF `orderedList` with three items,
  **When** I run `jira get PROJ-1`,
  **Then** the items render `1. …`, `2. …`, `3. …` (not `- …`); a `bulletList` still renders `- …`.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | An ADF `orderedList` flattens to 1-based numbered items (`1. `, `2. `, `3. `); the existing bulletList `- ` behavior is unchanged | test |
| AC2 | behavior | A nested `orderedList` inside a list item numbers independently from its parent (each level restarts at 1) | test |
| AC3 | constraint | No superfluous comments / banners / commented-out code; the dead `_ordered` parameter is gone (no `_`-prefixed unused param left); cyclomatic ≤10 / cognitive within ceiling | command (comment_policy + complexity) |

## Out of scope

- Markdown-faithful ordered-list continuation (`a.`, `i.`, lettered/roman markers) — Jira ADF only
  emits `order`-start integers; 1-based decimal is sufficient.
- Re-numbering from an ADF `order` start attribute other than 1 (rare; deferred until a real issue
  needs it).

## blocked_by

(none — isolated to the ADF flattener)
