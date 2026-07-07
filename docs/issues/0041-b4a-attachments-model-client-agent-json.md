---
type: Issue
title: "B4a — attachments on the model + client parse + agent_json array"
description: Curated Attachment{filename, url, mime_type, size} model; Issue gains #[serde(default)] attachments (cache back-compat by construction); map_gouqi_issue extracts fields.attachment tolerantly (absent/malformed -> empty); agent_json issue_object gains an additive attachments array. Demoable via `jira get --agent-json`.
status: done
labels: [model, client, agent-json, attachments, parity]
blocked_by:
tracker:
timestamp: 2026-07-06T00:00:00Z
---

## B4a — attachments data layer + agent_json

Implements [PRD 0003](/prd/0003-active-collab-parity.md) R-B4 (data half) per
[ADR 0020](/adr/0020-issue-attachments-detail-panel.md), behaviors
[BDR 0012](/bdr/0012-attachments-behaviors.md) S1–S2.

Delivery note: adding the mandatory `attachments` field to `Issue` broke the
three pre-[0027](/issues/0027-h1-test-support-module-and-adf-issue-builders.md)
hand-rolled `Issue{...}` fixtures (commands / models / store-cache tests) —
fixed mechanically here; the H-slices (0028/0029) remain the structural fix
for this recurring friction.
