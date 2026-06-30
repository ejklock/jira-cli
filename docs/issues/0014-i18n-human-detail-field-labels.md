---
type: Issue
title: "i18n: translate human-render field labels (get + browse detail)"
description: render_issue_human (CLI get), render_comment_human, and the TUI view_detail render field labels (Status:/Type:/Assignee:/…) and the Unassigned/Unknown fallbacks as raw English, while the table headers and the list Unassigned already go through t(). Route every human field label through t() with shared keys so the get output and the browse detail are both translatable and consistent.
status: done
tracker:
tags: [i18n, render, tui, debt]
timestamp: 2026-06-30T00:00:00Z
---

# i18n: translate human-render field labels (get + browse detail)

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R8 (i18n: en + pt-BR human output) and
[ADR 0006](/adr/0006-i18n-interpolation-contract.md). Closes the gap surfaced by the browse-TUI
final review (run 2585): the table headers (`KEY/TYPE/STATUS/ASSIGNEE/SUMMARY`) and the list
`Unassigned` are translated, but the per-field detail labels are not — in BOTH the CLI `get` human
render AND the TUI `view_detail`.

## Context manifest

- **Read first:** `src/render.rs` — `render_issue_human` (L187): raw labels `Status:` (L211),
  `Type:` (L216), `Priority:` (L219), `Assignee:` (L223), `Reporter:` (L224), `Created:` (L226),
  `Updated:` (L229), `Description:` (L232), and the `.unwrap_or("Unassigned")` fallback (L199);
  `render_comment_human` (L242): the `Comments:` header (L235) and the `.unwrap_or("Unknown")` author
  fallback (L243). `render_issue_table` (L150) is the EXISTING `t()` precedent (`t("KEY")` … `t("Unassigned")`).
- `src/tui.rs` — `view_detail` (L634): the `body` format! (L672) with raw `Status:`/`Type:`/
  `Assignee:`/`Description:` labels and the `.unwrap_or("Unassigned")` fallback (L659).
- `src/i18n.rs` — `t(s)` (L77) is identity in `en` (returns the key unchanged), so wrapping a label
  in `t()` leaves the en output byte-identical; only the pt-BR catalog adds a translation.
- `locales/pt_BR.json` — EXISTING keys to reuse: `"Status"`, `"Assignee"` (→ "Responsável"),
  `"Description"` (→ "Descrição"), `"Unassigned"` (→ "Não atribuído"). MISSING keys to add (below).

## Approach (decided)

- **Wrap every human field label in `t()`** in `render_issue_human`, `render_comment_human`, and
  `view_detail`, using the **bare word** as the key and appending the `:` / formatting in code —
  e.g. `format!("  {}: {} ({})", t("Status"), issue.status, …)`, `writeln!(out, "  {}: {}", t("Type"),
  issue.issue_type)`, and in `view_detail` `format!("{}: {}", t("Status"), status_line)` etc. Replace
  the raw `.unwrap_or("Unassigned")` / `.unwrap_or("Unknown")` with `t("Unassigned")` / `t("Unknown")`.
- **Reuse the existing keys** (`Status`, `Assignee`, `Description`, `Unassigned`); do not duplicate.
- **Add the missing pt-BR keys** to `locales/pt_BR.json` (en stays identity, no en catalog):
  `"Type": "Tipo"`, `"Priority": "Prioridade"`, `"Reporter": "Relator"`, `"Created": "Criado"`,
  `"Updated": "Atualizado"`, `"Comments": "Comentários"`, `"URL": "URL"`, `"Unknown": "Desconhecido"`.
- **`view_detail` reuses the same keys** as the CLI render — one home per label, so the two surfaces
  never drift.
- The `-` placeholders for missing optional values (status_category/priority/reporter) stay as `-`
  (not words, not translated).

## Vertical Demo

- **Given** language `pt_BR` (`jira setup language pt-BR`),
  **When** I run `jira get PROJ-1`,
  **Then** the labels read `Status:`, `Tipo:`, `Prioridade:`, `Responsável:`, `Relator:`, `Criado:`,
  `Atualizado:`, `Descrição:`, `Comentários:` and an unassigned issue shows `Não atribuído`.
- **And** in `jira browse`, opening the same issue's detail shows the same translated labels.
- **Given** language `en`, the output is byte-identical to today (t() identity).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | Under `pt_BR`, `render_issue_human` emits the translated labels (Tipo/Prioridade/Responsável/Relator/Criado/Atualizado/Descrição/Comentários) and `Não atribuído` for an unassigned issue | test |
| AC2 | behavior | Under `pt_BR`, the TUI `view_detail` buffer shows the translated labels (Status/Tipo/Responsável/Descrição) and `Não atribuído` for an unassigned issue | test |
| AC3 | behavior | Under `en`, `render_issue_human` and `view_detail` output is byte-identical to before (t() identity) — existing en render tests stay green | test |
| AC4 | constraint | Labels use shared `t()` keys (reused Status/Assignee/Description/Unassigned + the new keys); no duplicated label string; comment_policy clean; cyclomatic ≤10 / cognitive within ceiling | command (comment_policy + complexity) |
| AC5 | constraint | Every new language-dependent test holds a module-level `LANG_MUTEX`, calls `set_language` explicitly, and resets to `"en"` before returning (lesson 3331); render tests reuse the existing render `LANG_MUTEX`, the tui test adds a module-level one following the same convention | inspection (Reviewer) |

## Out of scope

- Consolidating the per-module `LANG_MUTEX` statics (commands/i18n/render) into one crate-wide lock —
  separate test-infra debt; this slice follows the established per-module convention.
- Translating the comment author/body chrome beyond the `Comments:` header + `Unknown` fallback.
- Any agent_json / `--json` change (the JSON contract is not localized).

## blocked_by

(none)
