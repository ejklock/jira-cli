---
type: BDR
title: "Body links: inline visible '[url]', Ctrl/Cmd+click opens, plain click never navigates"
description: Observable behavior of ADF links in the TUI detail — anchor text renders normal followed by a link-styled '[url]' token (mailto shows the bare address); keyboard Tab/Enter keeps working on the visible token; Ctrl/Cmd+click on any fragment of the token (even wrapped) opens the full URL; a plain click never navigates. Includes the Test Design matrix.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, links, mouse, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0010. Inline body-link behaviors

Realizes [ADR 0018](/adr/0018-inline-body-links-modifier-click.md) (port of
fork-base BDR 0014, amended D1c). S1–S4 land with slice B2a, S5–S8 with B2b.

## Scenarios

### S1 — Text and URL differ
**Given** an ADF link node `text="docs"`, `href="https://x/y"`,
**When** the detail renders, **Then** `docs [https://x/y]` appears — `docs` as
normal body text (other marks preserved), the bracketed token link-styled and
the only href carrier.

### S2 — Text equals URL, or is empty
**Given** anchor text equal to the href (or empty),
**When** rendered, **Then** only `[https://x/y]` appears (no duplication).

### S3 — mailto renders the bare address
**Given** `href="mailto:a@b.com"` with text `mail`,
**When** rendered, **Then** `mail [a@b.com]` appears and the token's stored
href remains `mailto:a@b.com` (Enter/click open the mailto).

### S4 — Keyboard nav focuses the visible token
**Given** a rendered detail with links, **When** the operator Tabs and presses
Enter, **Then** focus highlights the `[url]` token (REVERSED) and Enter emits
`Cmd::OpenUrl` with that href — the A2 contract, unchanged, one entry per link.

### S5 — Ctrl/Cmd+click on the token opens it
**Given** a rendered `text [url]`, **When** a click carrying CONTROL or SUPER
lands on any column of the `[url]` token, **Then** `Cmd::OpenUrl(href)` is
emitted (scroll offset honored).

### S6 — Plain click never navigates
**Given** the same token, **When** an unmodified click lands on it,
**Then** no open `Cmd` is emitted (reserved for text selection).

### S7 — A wrapped fragment opens the full URL
**Given** a URL long enough to wrap across panel lines,
**When** a Ctrl/Cmd+click lands on ANY wrapped fragment,
**Then** the open `Cmd` carries the COMPLETE href (span identity survives
wrapping) — and the full URL is visible on screen across the fragments.

### S8 — Ctrl/Cmd+click elsewhere is a no-op
**Given** the detail screen, **When** a modifier-click lands on non-link text,
panel borders, or chrome, **Then** nothing happens (no open, no panic); on the
list screen the modifier-click behaves like B1's plain click.

## Test Design

| Scenario | Level | Technique | Instrument / assertion |
|---|---|---|---|
| S1 | unit (render) | example | `adf_to_rich` output: normal-text span + link-styled `[url]` span carrying href; anchor span has no href |
| S2 | unit (render) | boundary | text==url and text=="" → single `[url]` span |
| S3 | unit (render) | example | `mail [a@b.com]` visible; span href == `mailto:a@b.com` |
| S4 | unit (pure model) | example | `description_link_hrefs` yields one href per link; existing FocusNextLink/Select tests still green (contract unchanged) |
| S5 | unit (geometry) | example | `detail_link_at` at a token column (with scroll offset) returns the href; update path emits `Cmd::OpenUrl` |
| S6 | unit | example | unmodified click intent on the same coordinates produces no open `Cmd` |
| S7 | unit (geometry) | example | wrapped token: `detail_link_at` on the second fragment's row returns the full href |
| S8 | unit (geometry) | boundary | border/plain-text/out-of-panel coordinates → None; list screen unaffected |

Rendering and geometry are pure (TestBackend/unit); the resolver recomputes the
renderer's own pipeline, so the render tests double as geometry oracles.

## References

- ADR: [/adr/0018-inline-body-links-modifier-click.md](/adr/0018-inline-body-links-modifier-click.md)
- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B3.
- Issues: [/issues/0038-b2a-inline-link-rendering.md](/issues/0038-b2a-inline-link-rendering.md), [/issues/0039-b2b-modifier-click-link-activation.md](/issues/0039-b2b-modifier-click-link-activation.md)
