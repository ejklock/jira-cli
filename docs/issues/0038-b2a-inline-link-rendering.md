---
type: Issue
title: "B2a — inline body-link rendering: 'text [url]' with a visible link-styled token"
description: ADF link marks in the rich mapper render the anchor text as normal body text followed by a link-styled '[url]' token that is the single href carrier; text==url/empty collapses to '[url]'; mailto shows the bare address. Keyboard nav (A2) rides the same span contract. adf_to_plain_text/agent_json untouched.
status: done
labels: [tui, render, links, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B2a — inline body-link rendering

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B3 (render half) per
[ADR 0018](/adr/0018-inline-body-links-modifier-click.md), behaviors
[BDR 0010](/bdr/0010-inline-body-link-behaviors.md) S1–S4.

**Known follow-ups (review observations):** links inside ADF table cells build
through a separate path and do not get the token (extend if real-world tables
carry links); `theme::link()` color is dead code — wiring it into the view's
span styling is folded into B2b (which touches `view.rs`).
