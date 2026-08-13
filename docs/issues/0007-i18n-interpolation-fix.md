---
type: Issue
title: "i18n: fix format-then-translate so interpolated chrome translates"
description: Replace the t(&format!(...)) call sites with a translate-then-substitute tf() primitive so interpolated chrome (instance errors, setup remove, setup language) actually renders in pt-BR. No new catalog keys.
status: done
tracker:
tags: [i18n, pt-BR, interpolation, debt]
timestamp: 2026-06-30T00:00:00Z
---

# i18n: fix format-then-translate so interpolated chrome translates

## Objective link

[ADR 0006](/adr/0006-i18n-interpolation-contract.md) (the interpolation contract) →
closes the format-then-translate debt left by J5 ([issue 0006](/issues/0006-j5-i18n.md)).
Traces up to [PRD 0001](/prd/0001-jira-cloud-read-cli.md) R8 (en + pt-BR chrome).

## Context manifest

- **Read first:** `src/i18n.rs` (the `t()` seam + catalog load), `locales/pt_BR.json`
  (templates already carry `{placeholder}` tokens), `src/commands.rs` (the eight
  `t(&format!(...))` sites), `tests/unit/i18n.rs` (the `LANG_MUTEX` pattern).
- **The defect:** `t(&format!("...{name}...", name = v))` interpolates *before* the
  catalog lookup, so the rendered string is never a `{placeholder}` key → silent
  English fallback under `pt_BR`. See ADR 0006.
- **The fix:** add `tf(template, &[(name, value)])` to `src/i18n.rs` (translate the
  template via `t`, then single-pass `\{(\w+)\}` substitution, regex cached in a
  `OnceLock`); convert all eight sites to `tf`. Every affected template already has
  a matching key in `pt_BR.json` with identical placeholder names — **no new catalog
  keys**.
- **Eight sites in `src/commands.rs`:** `resolve_instance` "instance not found. Known"
  (~L39) and "multiple instances configured" (~L59); `setup_remove` "instance not
  found." (~L121) and "removed." (~L129); `setup_language` "Current language" (~L150),
  "Language set to" (~L164), "unsupported language" (~L174); and `setup_test`
  "instance not found." (~L206).
- **Determinism (lesson):** every test whose assertion depends on the active language
  MUST acquire the shared module-level `LANG_MUTEX`, `set_language` explicitly, and
  reset to `en` before returning — the process-global language races parallel tests
  otherwise. `tf` is synchronous, so no `!Send` guard-across-`.await` problem.

## Vertical Demo

- **Given** `JIRA_CLI_LANG=pt-BR` (or stored setting `pt_BR`),
  **When** I run `jira get NOPE` against a config whose instance name does not match,
  **Then** the "instance not found" error renders in Portuguese
  (`Erro: instância '…' não encontrada.`), not English.
- **Given** the language is `pt_BR`,
  **When** I run `jira setup language pt-BR`,
  **Then** the confirmation renders `Idioma definido como 'pt_BR'.`
- **Unhappy path:** **When** I run `jira setup language zz` under `pt_BR`,
  **Then** the unsupported-language error renders in Portuguese and exits 2.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | behavior | Under `pt_BR`, `tf(template, args)` returns the **translated** template with `{name}` tokens substituted (e.g. instance-not-found → `Erro: instância 'foo' não encontrada.`); an unknown placeholder is left intact; an unknown template falls back to the input template with substitution applied | test |
| AC2 | behavior | Under `en`/identity, `tf` returns the template with `{name}` tokens substituted and English text preserved; a value containing a `{token}` is **not** re-interpreted (single-pass) | test |
| AC3 | constraint | All eight `t(&format!(...))` sites in `src/commands.rs` are converted to `tf`; **no `t(&format!(` occurrence remains** anywhere in `src/` | command (grep) |
| AC4 | constraint | No superfluous comments / banners / commented-out code; only non-obvious why-comments | inspection + comment_policy |
| AC5 | constraint | Cyclomatic ≤ 10 (≤ 8 for new `tf`) / cognitive within the gate ceiling | quality-gate complexity |
| AC6 | constraint | Mutants on changed lines killed; tests assert observable translated output, not implementation | quality-gate mutation (Reviewer backstop) |

## Out of scope

- Interpolated chrome that **never** attempted translation and has no catalog key
  (`get` issue-not-found, `search` `invalid JQL`, branch-no-key, `setup add`
  connectivity/saved lines) — deferred to a future J5d coverage slice.
- `render_issue_human` detail labels (`Status:`/`Type:`/`Priority:`), still English.
- Any new locale or new catalog key.

## blocked_by

[0006](/issues/0006-j5-i18n.md)
