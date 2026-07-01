---
type: Issue
title: "browse TUI chrome i18n parity — translate remaining footers/prompt via t() + pt_BR catalog"
description: Close the i18n parity gap in the browse TUI chrome. Several rendered strings (the JQL prompt, the error-banner prefix, and both list/search footers) are hardcoded English that never pass through t(); the P3 "n more" hint passes through t() but is missing from the pt_BR catalog. Wrap the remaining chrome in t() and complete the pt_BR catalog so `LANG=pt_BR jira browse` renders fully translated chrome with no English leaking. Jira data (issue rows, ADF, keys) is never translated.
status: done
tracker:
tags: [tui, browse, i18n, phase2, parity]
timestamp: 2026-07-01T00:00:00Z
---

# 0020 — browse TUI chrome i18n parity

## Objective link

[PRD 0001](/prd/0001-jira-cloud-read-cli.md) R6 (i18n: en + pt-BR) and
[PRD 0002](/prd/0002-interactive-browse-tui.md) (the browse TUI), realizing
[ADR 0006](/adr/0006-i18n-format-then-translate.md) (the `t()`/`tf()` seam). Sibling of
issue [0014](/issues/0014-i18n-human-detail-field-labels.md) (which translated the detail
field labels); this slice finishes the list/search chrome the same way. Part of the
browse-TUI **total-parity** program (the tool is complete only when pt_BR shows no English
chrome).

## Context manifest

- **Read first:** `src/tui/view.rs` — `view_list` and `view_detail`. The gaps:
  - `SEARCH_PROMPT = "JQL> "` (L13) rendered raw at L49 — never `t()`'d.
  - `SEARCH_ERROR_PREFIX = "Error: "` (L14) rendered raw at L56 — never `t()`'d.
  - the normal-list footer `"↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"` (L133)
    and the search footer `"Enter submit  Esc cancel  Backspace delete"` (L131) — raw, no `t()`.
  - `t("n more")` (L136) — already `t()`'d, but the key is absent from the catalog.
  - Already correct (do not touch): the detail footer `t("↑/↓ scroll  Esc/b back  q quit")`
    (L157), the header cells (`t("KEY")` …), `t("No issues.")`, `t("Unassigned")`,
    `t(LOADING_NOTICE)`.
- `locales/pt_BR.json` — the embedded pt_BR catalog (`include_str!` in `src/i18n.rs`). It
  already carries the detail footer, headers, `No issues.`, `Unassigned`. It does NOT carry
  the list/search footers used by jira-cli, `n more`, or `Error: `. (Note: the catalog still
  carries AC-era strings like `↑/↓ navigate  Enter select  r refresh …` — a *different*
  footer that does not match jira-cli's; do not reuse it. Cleaning dead AC keys is out of
  scope here.)
- `src/i18n.rs` — `t(s)`: identity under `en`, catalog lookup under `pt_BR` with identity
  fallback for unknown keys. `#[cfg(test)] pub(crate) LANG_MUTEX` serializes language-dependent
  tests (set it, then `set_language`).
- `tests/unit/tui.rs` — existing `update`/`TestBackend` tests; the pattern for a
  language-scoped render test is: lock `LANG_MUTEX`, `set_language("pt_BR")`, render via
  `TestBackend`, assert the buffer, restore `set_language("en")`.

## Approach (decided)

- In `view.rs`, route every remaining chrome string through `t()` at the render site:
  - `SEARCH_PROMPT`: render as `format!("{}{query}", t("JQL> "))`. `JQL` is a Jira proper
    noun — the pt_BR value stays `"JQL> "` (identity is acceptable; wrapping it just makes the
    seam uniform). No catalog entry required for it.
  - `SEARCH_ERROR_PREFIX`: render as `format!("{}{msg}", t("Error: "))`.
  - both footer literals: wrap the whole literal in `t(...)` before building the Paragraph.
  - keep `t("n more")` as-is.
- In `locales/pt_BR.json`, add the missing keys with Brazilian-Portuguese values:
  - `"↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"` → `"↑/↓ navegar  /  buscar  Enter selecionar  Esc/b voltar  q sair"`
  - `"Enter submit  Esc cancel  Backspace delete"` → `"Enter enviar  Esc cancelar  Backspace apagar"`
  - `"Error: "` → `"Erro: "`
  - `"n more"` → `"n mais"`
- Do NOT translate Jira data (rows, keys, ADF, comments) and do NOT translate the `JQL`
  proper noun.

## Vertical Demo

- **Given** `LANG=pt_BR jira browse` against a real instance,
  **When** the list renders and I press `/` to search then submit an invalid JQL,
  **Then** the footer reads `↑/↓ navegar  /  buscar  …`, the search footer reads
  `Enter enviar  Esc cancelar  Backspace apagar`, the error banner reads `Erro: …`, and
  when a next page is pending the hint shows `n mais` — with no English chrome leaking.
- **And** under the default `en` locale every string is byte-identical to today (identity).

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | Under `pt_BR`, `view_list` renders the translated normal-list footer, search footer, error-banner prefix (`Erro: `), and the `n mais` hint (when a token is pending) — asserted via `TestBackend` buffer | test (TestBackend) |
| AC2 | behavior | Under `en`, `view_list` output is byte-identical to the pre-change render (identity) — no regression — asserted via `TestBackend` | test (TestBackend) |
| AC3 | constraint | Every chrome string in `view_list` passes through `t()` (no raw English rendered except the `JQL` proper noun, which is `t()`-wrapped with identity value); Jira data is never passed to `t()` | inspection |
| AC4 | constraint | `locales/pt_BR.json` stays valid JSON (parses in `pt_br_catalog()`); the four new keys are present with pt-BR values | test (i18n catalog load) |
| AC5 | constraint | clippy `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --test comment_policy` clean; cyclomatic ≤10 / cognitive within ceiling; a mutant that drops a `t()` wrapper (rendering English under pt_BR) is killed by AC1; no superfluous comments/banners/commented-out code | command (clippy + fmt + comment_policy + complexity) |

## Out of scope

- Cleaning dead active-collab-cli keys from `locales/pt_BR.json` (Tasks/Projects/Assets/
  ActiveCollab descriptions, etc.) — a separate catalog-hygiene slice.
- Any appearance change (colors, borders, layout, column widths) — this slice is text/i18n only.
- Broader active-collab-cli feature parity (needs the AC-cli source as reference) — tracked
  separately.

## blocked_by

(none — additive i18n completion on the delivered browse TUI)
