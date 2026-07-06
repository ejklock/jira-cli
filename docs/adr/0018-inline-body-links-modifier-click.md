---
type: ADR
title: Inline body links ('text [url]') with modifier-gated click activation
description: ADF links in the TUI detail render the anchor text as normal body text followed by a visible, link-styled '[url]' token (the single href carrier); a Ctrl/Cmd+click on any fragment of that token opens the URL via a pure, stateless hit resolver that recomputes the renderer's own geometry. Plain click never navigates (reserved for text selection). The plain-text/agent_json path is untouched.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, links, mouse, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0018. Inline body links + modifier-gated click activation

## Context

Today an ADF link renders as underlined link-colored anchor text with the href
retained invisibly on the span (`RichSpan.style.link`, ADR 0010); activation is
keyboard-only (Tab cycle + Enter, ADR 0011). The fork base moved to inline
`text [url]` rendering with Ctrl/Cmd+click activation (its ADR 0020 + BDR 0014,
amended D1c) after learning two things the hard way: an **invisible URL** is
neither verifiable nor copyable, and **index-indirected click mapping** ("Link
N") misses. [PRD 0003](/prd/0003-active-collab-parity.md) R-B3 ports that end
state. B1 (ADR 0017) already delivers the mouse plumbing; detail clicks are
currently a no-op.

## Decision

Delivered as two slices: **B2a — inline render**, **B2b — click activation**.

### B2a — inline `text [url]` in the rich mapper only

1. In `adf_to_rich`, a link mark no longer styles the anchor text: the text
   renders as normal body text (other marks preserved), followed by a ` [url]`
   token span — the **only** span carrying `style.link = href`, styled with the
   theme link style. Anchor text empty or equal to the URL → only `[url]`.
   `mailto:` hrefs render the bare address in the brackets; the stored href
   keeps the scheme (open re-adds nothing).
2. `description_link_hrefs` and the A2 keyboard nav are untouched by contract:
   they collect/focus href-carrying spans in render order — now exactly one
   visible `[url]` token per link (the focus REVERSED highlight lands on the
   visible token; strictly better UX, same code path).
3. **`adf_to_plain_text` is untouched**: it feeds the frozen `agent_json`
   contract (ADR 0004) and the CLI plain render — same boundary the fork chose
   (its CLI path was explicitly unchanged).

### B2b — Ctrl/Cmd+click activation, pure and stateless

4. B1's mouse mapper gains the click's modifier set. On the Detail screen a
   click **with CONTROL or SUPER** resolves through a pure
   `view::detail_link_at(model, area, x, y) -> Option<String>` and, on a hit,
   feeds a `Msg` that emits the existing `Cmd::OpenUrl`. A **plain click never
   navigates** (fork D1c amendment: reserved for the upcoming app-managed text
   selection; accidental navigation is worse than an extra modifier).
5. `detail_link_at` is **stateless single-source geometry**: it recomputes the
   same `build_detail_lines` → wrap → `clamp_scroll_offset` → panel-chrome
   pipeline the renderer uses and returns the href of the span under (x, y).
   No hit-target cache, no shell state — the fork's "hit targets emitted
   structurally" lesson without its cache machinery (recompute-on-click is
   trivially cheap at TUI scale).
6. **Wrapped fragments resolve the full URL for free**: wrapping preserves
   span identity (`wrap_line_to_width` clones styles), so any fragment of a
   wrapped `[url]` token carries the complete href (fork BDR 0014 S7).

## Alternatives considered

- **Plain click opens links.** Rejected (fork D1c): bare click is reserved for
  selection/cursor placement; terminals' own convention is modifier+click.
- **Keep invisible hrefs + add click on the anchor text.** Rejected: the URL
  stays unverifiable/uncopyable — the fork's original defect.
- **Cache hit targets from the last rendered frame in the shell.** Rejected:
  state that can go stale (scroll between draw and click) for no measurable
  win; the pure recompute uses the same fns and cannot drift.
- **Inline `[url]` in `adf_to_plain_text` too.** Rejected: the agent_json
  contract is frozen (ADR 0004) and the fork kept its CLI unchanged.

## Consequences

**Positive:** URLs become visible, copyable, and clickable exactly where they
appear; keyboard nav (A2) keeps working on the same code path; the resolver
cannot drift from the renderer (single geometry source).

**Accepted trade-offs:** body text grows by the URL length (wrapped, never
truncated); link activation needs a modifier; a second full-geometry recompute
per modifier-click (negligible).

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B3.
- BDR: [/bdr/0010-inline-body-link-behaviors.md](/bdr/0010-inline-body-link-behaviors.md)
- ADR: [/adr/0010-styled-adf-rendering-browse-tui-detail.md](/adr/0010-styled-adf-rendering-browse-tui-detail.md), [/adr/0011-keyboard-inline-link-navigation-browse-detail.md](/adr/0011-keyboard-inline-link-navigation-browse-detail.md), [/adr/0017-mouse-support-browse-tui.md](/adr/0017-mouse-support-browse-tui.md)
- Fork base: ADR 0020 + BDR 0014 (amended D1c).
- Issues: [/issues/0038-b2a-inline-link-rendering.md](/issues/0038-b2a-inline-link-rendering.md), [/issues/0039-b2b-modifier-click-link-activation.md](/issues/0039-b2b-modifier-click-link-activation.md)
