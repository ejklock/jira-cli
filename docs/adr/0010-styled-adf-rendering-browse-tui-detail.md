---
type: ADR
title: "Styled ADF rendering in the browse TUI detail (neutral rich model + ratatui mapping)"
description: Render the browse TUI detail's ADF description with inline styling (bold/italic/code/strike/underline/link) instead of flattening to plain text. Introduce a pure, ratatui-free rich model (RichSpan/RichLine) produced by a new adf_to_rich walker; the view maps it to ratatui spans. The existing adf_to_plain_text stays byte-stable for the CLI get render and agent_json. This is the reusable primitive that clickable links (A2) and comment display/nav (A4) build on.
status: Accepted
supersedes:
superseded_by:
tags: [tui, browse, render, adf, parity, phase2]
timestamp: 2026-07-01T00:00:00Z
---

# 0010. Styled ADF rendering in the browse TUI detail

## Context

`jira-cli` targets **total feature parity** with its fork base `active-collab-cli`
(AC), swapping only the domain core and respecting Jira's specifics
([ADR 0001](/adr/0001-fork-active-collab-cli-swap-api.md)). AC renders issue bodies
and comments as **styled rich text** (`src/richtext.rs`: `RichStyle`/`RichSpan`/
`RichLine`, an HTML subset → styled spans — bold, italic, code, strike, underline,
link). `jira-cli` today only has `adf_to_plain_text` (`src/render.rs:14`): a
recursive ADF walker that **drops every inline `mark`** and emits a flat `String`,
rendered in the TUI detail as one plain `Paragraph`. The user's `get` output and
`agent_json` contract depend on that plain flattening and are locked by tests.

Force: parity is a **read-only display** gap first (the user chose "read-only now,
writes later"). The browse TUI detail should show Jira's rich formatting the way AC
showed ActiveCollab's — but Jira uses **ADF** (Atlassian Document Format, a JSON
tree with `marks` on text nodes), not HTML, so the rich model must be re-derived
for ADF, not copied from AC.

ADF inline marks to honor (on `text` nodes, `marks: [{ type, attrs? }]`):
`strong` → bold, `em` → italic, `code` → code, `strike` → strikethrough,
`underline` → underline, `link` → underline + retained `href` (the href is what a
later clickable-links slice (A2) needs).

## Decision

Add a **pure, ratatui-free rich model** and a parallel ADF walker; map it to
ratatui only in the view (Humble Object).

1. **Neutral rich model** in `src/render.rs` (no ratatui dependency, so it stays in
   the functional core and is unit-testable — NFR-B1/B2 of
   [PRD 0002](/prd/0002-interactive-browse-tui.md)):
   - `RichStyle { bold: bool, italic: bool, code: bool, strike: bool, underline: bool, link: Option<String> }`
   - `RichSpan { text: String, style: RichStyle }`
   - `RichLine = Vec<RichSpan>`
2. **`adf_to_rich(raw: &str) -> Vec<RichLine>`** — walks the same ADF structure as
   `adf_to_plain_text` (paragraph/heading/list/codeBlock/blockquote/hardBreak/…) but
   accumulates inline `marks` into `RichStyle` per `text` run. Non-ADF input (the
   `raw.to_string()` fallback path) yields a single unstyled line. Block structure
   (list markers, indentation, code blocks) matches the plain walker's shaping so
   the two stay visually consistent.
3. **`adf_to_plain_text` is unchanged and stays byte-stable** — the CLI `get` human
   render and `agent_json` keep flattening to plain text (their tests lock the
   output). The rich path is **TUI-only**.
4. **The view maps `Vec<RichLine>` → ratatui `Text`/`Line`/`Span`** in `view_detail`
   (`src/tui/view.rs`): `bold`→`Modifier::BOLD`, `italic`→`ITALIC`, `strike`→
   `CROSSED_OUT`, `underline`→`UNDERLINED`, `code`→a dim/distinct style,
   `link`→`UNDERLINED` (the `href` is retained in the model for A2, not yet
   clickable). The metadata lines (key/summary/status/type/assignee labels) stay as
   today. `Paragraph::scroll` still drives detail scrolling over the styled `Text`.

## Scope

- **This decision covers A1**: styled rendering of the **description** in the TUI
  detail. It deliberately does **not** wire clickable links (A2), nor display/
  navigate comments in the TUI detail (A4) — those are later read-only slices that
  reuse this `adf_to_rich` primitive.

## Alternatives considered

- **Extend `adf_to_plain_text` to also return styles.** Rejected: it is a locked,
  byte-stable seam for the CLI/agent_json; overloading it risks regressing those
  contracts. A parallel walker keeps the plain path frozen.
- **Build ratatui `Text` directly in the walker.** Rejected: couples the functional
  core to ratatui, breaking the pure/testable boundary (NFR-B1) and the Humble-Object
  split ([ADR 0007](/adr/0007-browse-tui-elm-architecture.md)). The neutral model is
  unit-testable with no backend.
- **Vendor AC's `richtext.rs` as-is.** Rejected: AC parses **HTML**; Jira is **ADF**
  (a JSON tree). The model (RichSpan/RichLine) is worth mirroring; the parser is not.

## Consequences

**Positive:**

- The TUI detail shows Jira formatting (bold/italic/code/links styled), a real parity
  gain, with the pure core still unit-testable and the ratatui mapping isolated to the view.
- `adf_to_rich` is the reusable primitive for A2 (clickable links — href already retained)
  and A4 (styled comment display/nav).

**Accepted trade-offs:**

- Two ADF walkers (plain + rich) now share structure. The plain one must stay
  byte-stable; a shared private traversal core may be factored **only if** it does not
  perturb the plain output. Minor duplication is acceptable over risking the locked contract.
- `view_detail` moves from a single flat `Paragraph` string to a styled `Text`; the
  metadata composition changes shape (still scrollable).

## Related

- Constitution: [/constitution.md](/constitution.md)
- PRD: [/prd/0002-interactive-browse-tui.md](/prd/0002-interactive-browse-tui.md)
- ADR: [/adr/0007-browse-tui-elm-architecture.md](/adr/0007-browse-tui-elm-architecture.md)
- ADR: [/adr/0001-fork-active-collab-cli-swap-api.md](/adr/0001-fork-active-collab-cli-swap-api.md)
- Issue: [/issues/0021-a1-styled-adf-detail-rendering.md](/issues/0021-a1-styled-adf-detail-rendering.md)
