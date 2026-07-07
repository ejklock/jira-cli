---
type: ADR
title: Issue attachments — curated model field, agent_json array, and an inline Attachments panel in the detail
description: Parse Jira fields.attachment into a curated Attachment model (filename, content URL, mime type, size) carried on Issue with serde back-compat; expose it additively in agent_json; render an Attachments panel after Comments inside compose_detail's one pass, with '[n] ↗ filename' rows whose cells carry the href — so B2b modifier-click activation and B3 selection work on attachment rows with zero new click machinery. Filename is the label (Jira gives real filenames; the fork's anchor-text→filename→host derivation collapses).
status: Accepted
supersedes:
superseded_by:
tags: [model, client, agent-json, tui, detail, attachments, parity]
timestamp: 2026-07-06T00:00:00Z
---

# 0020. Issue attachments end-to-end

## Context

[PRD 0003](/prd/0003-active-collab-parity.md) R-B4 ports the fork's asset
surface. The fork's end state (its ADR 0029/BDR 0022, after superseding a
fixed capped panel): assets render **inline in the one globally-scrollable
detail content** — a localized header, `[n] ↗ label` rows, a blank row
between consecutive rows, an italic Ctrl/Cmd+click footnote — every asset
reachable by scrolling, activation via the same scroll-aware click machine as
body links. Its label derivation (anchor text → real filename → host, ADR
0023) existed because assets were scraped from body HTML.

Jira is structurally better off: `fields.attachment` is **first-class data**
with a real `filename`, `content` URL, `mimeType`, and `size` — no scraping,
no label heuristics. And this repo already has (B2b/B3) a single-geometry
detail compose whose cells carry `href` provenance, driving both
modifier-click activation and text selection.

## Decision

1. **Model:** a curated `Attachment { filename, url, mime_type, size }`; the
   `Issue` gains `#[serde(default)] attachments: Vec<Attachment>` (same
   back-compat pattern as `comments`/`duedate` — old cached rows keep
   deserializing; the cache needs no migration).
2. **Client:** `map_gouqi_issue` extracts `fields.attachment` (array) via the
   same raw-fields pattern as `extract_duedate`; missing/malformed → empty
   vec, never an error. The list/search path stays `IssueRow` (no
   attachments) — detail is fetched on Enter as today.
3. **agent_json (amends ADR 0004 additively):** `issue_object` gains an
   `attachments` array of `{filename, url, mime_type, size}` shaped like
   `shape_comment` — same additive precedent as `duedate` (ADR 0013). No
   existing key changes.
4. **TUI:** an **Attachments panel** after Comments, composed inside
   `compose_detail`'s single pass: localized `Attachments (N)` header,
   `[n] ↗ filename` rows with the theme link style and `href` = content URL
   in the cell metadata, one blank row between consecutive rows (fork
   breathing room), and an italic/dim Ctrl/Cmd+click footnote as the last
   panel line. **No new click or selection machinery:** because the cells
   carry `href`, B2b's `detail_link_at` activation (via the existing
   `Cmd::OpenUrl`) and B3's selection/extraction work on attachment rows by
   construction; the panel joins the one global scroll, so no attachment is
   ever clipped (the fork's ceiling bug class cannot exist here).
5. **Label = filename.** The fork's three-step derivation collapses: Jira
   guarantees a real filename. Body web links are already first-class inline
   `[url]` tokens (ADR 0018) — they never masquerade as attachments.
6. **Empty list renders no panel** — no header, no footnote.
7. **Sliced** B4a (model + client + agent_json) and B4b (TUI panel), each
   demoable (`jira get --agent-json`; the browse detail).

## Alternatives considered

- **A fixed bottom attachments panel.** Rejected: the fork shipped it, hit
  the silent-clipping defect, and superseded it (its ADR 0024 → 0029). We
  port the end state, not the detour.
- **A new `Cmd::OpenAsset`.** Rejected: the content URL opens through the
  existing `Cmd::OpenUrl`/browser seam; a second open command duplicates a
  contract for no behavioral difference.
- **Fork-style label derivation (anchor → filename → host).** Rejected:
  heuristics for a problem Jira doesn't have.
- **Attachments in the CLI `get` human output.** Deferred: not part of the
  fork's surface (its CLI never rendered assets); agent_json covers the
  machine consumer. Can be a follow-up slice if wanted.

## Consequences

**Positive:** attachments visible and openable in the detail with zero new
geometry/click code; agent consumers get the full curated attachment list;
cache back-compat by construction; the fork's clipping bug class is
structurally impossible.

**Accepted trade-offs:** opening a content URL in the browser relies on the
user's Jira web session for the download (same trust model as every other
opened link); `size`/`mime_type` are carried but not yet rendered in the TUI
row (filename-first parity; available for a future row enrichment).

## Related

- PRD: [/prd/0003-active-collab-parity.md](/prd/0003-active-collab-parity.md) — R-B4.
- ADR: [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md) (amended additively), [/adr/0013-relative-due-date-rendering.md](/adr/0013-relative-due-date-rendering.md) (the additive precedent), [/adr/0018-inline-body-links-modifier-click.md](/adr/0018-inline-body-links-modifier-click.md) + [/adr/0019-app-managed-text-selection.md](/adr/0019-app-managed-text-selection.md) (the machinery attachments reuse).
- BDR: [/bdr/0012-attachments-behaviors.md](/bdr/0012-attachments-behaviors.md)
- Fork base: ADR 0023/0024/0027/0028/0029/0032, BDR 0017–0019/0021/0022.
- Issues: [/issues/0041-b4a-attachments-model-client-agent-json.md](/issues/0041-b4a-attachments-model-client-agent-json.md), [/issues/0042-b4b-attachments-tui-panel.md](/issues/0042-b4b-attachments-tui-panel.md)
