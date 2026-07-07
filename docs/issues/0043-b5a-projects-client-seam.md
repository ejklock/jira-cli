---
type: Issue
title: "B5a — projects client seam: ProjectRow model + JiraClient::list_projects"
description: Curated ProjectRow{key, name}; new JiraClient trait method list_projects() over GET /rest/api/3/project/search (single page, up to 100), tolerant mapping (skip malformed entries), auth errors classified like the other calls. Layer-shaped seam slice (P2 precedent) — B5b makes it observable.
status: open
labels: [client, model, projects, parity]
blocked_by:
tracker:
timestamp: 2026-07-07T00:00:00Z
---

## B5a — projects client seam

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B5 (data half) per
[ADR 0021](/adr/0021-projects-axis-browse.md), behaviors
[BDR 0013](/bdr/0013-projects-axis-behaviors.md) (client rows of the matrix).
