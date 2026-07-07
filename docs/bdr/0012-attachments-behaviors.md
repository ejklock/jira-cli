---
type: BDR
title: Issue attachments — agent_json array and inline detail panel behaviors
description: Observable behaviors for issue attachments — the curated agent_json attachments array, and the browse detail's Attachments panel ('[n] ↗ filename' link rows, blank-row breathing room, italic Ctrl/Cmd+click footnote, modifier-click opens at any scroll position, plain click never opens, selection works over rows, empty list renders no panel).
status: Accepted
supersedes:
superseded_by:
tags: [attachments, agent-json, tui, detail, behavior]
timestamp: 2026-07-06T00:00:00Z
---

# 0012. Attachments behaviors

Behaviors for [ADR 0020](/adr/0020-issue-attachments-detail-panel.md), ported
from the fork base's BDR 0022 (its inline end state) and BDR 0017/0019/0021,
adapted to Jira's first-class `fields.attachment`.

## Scenarios

### S1 — client parses attachments

- **Given** a Jira issue payload whose `fields.attachment` lists files
- **When** the issue is fetched
- **Then** the Issue carries each attachment's filename, content URL, mime
  type, and size; a payload without the field (or with a malformed entry)
  yields an empty list, never an error; old cached issues keep deserializing.

### S2 — agent_json carries the attachments array

- **Given** an issue with attachments
- **When** `--agent-json` output is produced
- **Then** it contains an `attachments` array of
  `{filename, url, mime_type, size}` objects; an issue without attachments
  yields an empty array; no pre-existing key changes.

### S3 — Attachments panel renders after Comments

- **Given** the browse detail of an issue with attachments
- **When** the detail renders
- **Then** an `Attachments (N)` panel (localized) appears after the Comments
  panel inside the same global scroll, one `[n] ↗ filename` link-styled row
  per attachment.

### S4 — breathing room and footnote

- **Given** two or more attachments
- **When** the panel renders
- **Then** one blank row separates consecutive rows, and the panel's last
  line is an italic/dim Ctrl/Cmd+click footnote (localized).

### S5 — every attachment reachable by scrolling

- **Given** more attachments than fit on screen
- **When** the user scrolls to the bottom
- **Then** every row is reachable — the panel is ordinary scrollable content
  with no height ceiling.

### S6 — modifier-click opens at any scroll position

- **Given** an attachment row visible at any scroll offset
- **When** the user Ctrl/Super+clicks it
- **Then** the attachment's content URL opens via the existing open-URL
  command; clicks on the header, blank rows, or the footnote open nothing.

### S7 — plain click never opens; selection works

- **Given** the Attachments panel
- **When** the user plain-clicks or drags over a row
- **Then** no URL opens; B3 selection/copy behaves over attachment rows
  exactly as over body text.

### S8 — empty list renders no panel

- **Given** an issue without attachments
- **When** the detail renders
- **Then** no Attachments header, rows, or footnote appear.

## Test Design

| Case | Level | Scenario | Asserts (observable) |
|---|---|---|---|
| Parse full/missing/malformed field | unit (wiremock) | S1 | Issue.attachments contents; empty on absent/malformed; cache round-trip |
| agent_json array shape | unit | S2 | attachments array objects; empty array; existing keys byte-identical |
| Panel after Comments | render (TestBackend) | S3 | header + `[n] ↗ filename` rows positioned after Comments panel |
| Blank rows + italic footnote | render | S4 | blank row between rows; last line italic/dim hint |
| Scroll reaches last row | render | S5 | max offset exposes the final attachment row |
| Modifier-click resolves href | unit + render | S6 | detail_link_at over an attachment row returns the content URL; header/footnote → None |
| Plain click / selection | unit | S7 | no open Cmd on plain click; selection_text extracts row text |
| Empty list | render | S8 | no panel lines appended; scroll bound unchanged |

## Related

- ADR: [/adr/0020-issue-attachments-detail-panel.md](/adr/0020-issue-attachments-detail-panel.md)
- BDR: [/bdr/0010-inline-body-link-behaviors.md](/bdr/0010-inline-body-link-behaviors.md), [/bdr/0011-detail-text-selection-behaviors.md](/bdr/0011-detail-text-selection-behaviors.md)
- Issues: [/issues/0041-b4a-attachments-model-client-agent-json.md](/issues/0041-b4a-attachments-model-client-agent-json.md), [/issues/0042-b4b-attachments-tui-panel.md](/issues/0042-b4b-attachments-tui-panel.md)
- Fork base: BDR 0022 (supersedes its 0018/0021), BDR 0017/0019.
