---
type: ADR
title: "i18n interpolation contract: translate the template, then substitute"
description: Interpolated chrome must translate the placeholder TEMPLATE through the catalog and then substitute runtime values, via a single tf(template, args) primitive. The t(&format!(...)) pattern is banned because it interpolates before lookup, so the runtime string never matches a {placeholder} catalog key and silently falls back to English.
status: Accepted
supersedes:
superseded_by:
tags: [i18n, pt-BR, en, interpolation, contract]
timestamp: 2026-06-30T00:00:00Z
---

# 0006. i18n interpolation contract: translate the template, then substitute

## Context

[ADR 0004](/adr/0004-agent-json-output-contract.md) keeps `agent_json` literal;
J5 ([issue 0006](/issues/0006-j5-i18n.md)) made the **human chrome** translatable
behind a single seam: `src/i18n.rs::t(key)` looks a string up in the active
catalog (`pt_BR.json`) and returns it, or the input unchanged under `en`/unknown
keys. The catalog stores **templates with named `{placeholder}` tokens**, e.g.

```json
"Error: instance '{name}' not found.": "Erro: instância '{name}' não encontrada.",
"Language set to '{code}'.": "Idioma definido como '{code}'."
```

The J5 slices shipped the interpolated call sites with a known defect, recorded as
debt: they wrap `t(&format!(...))` — `format!` substitutes the runtime values
**first**, producing e.g. `"Error: instance 'foo' not found."`, and only then does
`t()` look that string up. The interpolated string is never a catalog key, so the
lookup misses and the message silently falls back to English even under `pt_BR`.
This is the classic **format-then-translate** anti-pattern: translation is attempted
but can never succeed for any string carrying a runtime value.

Eight call sites in `src/commands.rs` (the `resolve_instance` errors, `setup remove`,
and `setup language`) are affected; every one of them has a matching `{placeholder}`
template already present in `pt_BR.json` with identical placeholder names — so the
fix needs **zero new catalog keys**, only the correct evaluation order.

## Decision

Adopt a **translate-then-substitute** interpolation contract with a single new
primitive in the `i18n` seam:

```rust
/// Translate `template` via the active catalog, THEN substitute `{name}` tokens.
pub fn tf(template: &str, args: &[(&str, &str)]) -> String
```

1. **Order is fixed: translate, then interpolate.** `tf` first calls `t(template)`
   (catalog lookup on the placeholder template — a real key), then replaces each
   `{name}` token in the translated string with its value. Under `en`/unknown keys
   `t` returns the template unchanged, so substitution still yields correct English.
2. **Single-pass substitution.** Token replacement scans the translated template
   **once** (a `\{(\w+)\}` pass), so a runtime value that happens to contain a
   `{token}` is never re-interpreted, and an unmatched placeholder is left intact
   rather than panicking. The compiled regex is cached in a `OnceLock`.
3. **`t(&format!(...))` is banned.** Interpolated chrome goes through `tf`; `t` is
   reserved for static strings. The two-seam split (`t` static, `tf` interpolated)
   is the whole contract — there is no third path.
4. **Catalog keys stay templates.** Catalog entries keep their `{placeholder}`
   tokens verbatim; they are the lookup keys `tf` translates against. Placeholder
   names in the catalog template and in the `tf` call must match.

## Alternatives considered

- **Inline `t("...{name}...").replace("{name}", v)` at each site.** Rejected: it
  re-implements the substitution at every call site (duplication), re-scans on each
  `replace` (re-interpretation hazard), and offers no single place to enforce the
  contract.
- **A formatting macro (`tformat!`) wrapping `format!`.** Rejected for v1: a macro
  that takes a literal template and named args is more machinery than a plain
  `&[(&str, &str)]` slice needs, and it obscures the translate-first ordering that
  is the entire point.
- **Translate fully-rendered strings (keep `t(&format!())`, add rendered keys to
  the catalog).** Rejected: the catalog would need one key per distinct runtime
  value (unbounded), which is not translation at all.

## Consequences

**Positive:**

- Interpolated chrome actually translates under `pt_BR`; the J5 debt is closed with
  no new catalog keys.
- One enforceable rule (`tf` for interpolated, `t` for static; never
  `t(&format!())`) the Reviewer can check mechanically.
- Single-pass substitution removes the value-contains-a-token footgun.

**Accepted trade-offs:**

- A second i18n primitive (`tf`) alongside `t`. Justified: the two map cleanly to
  static vs interpolated, and collapse the format-then-translate defect class.
- Interpolated chrome that is currently **English-only** (e.g. `get` issue-not-found,
  `search` `invalid JQL`, the branch-no-key line, the `setup add` connectivity/saved
  lines) is **not** migrated here — those never attempted translation and have no
  catalog keys. Closing that coverage gap is deferred to a future J5d slice; this ADR
  only fixes the **format-then-translate defect** on sites that already wrap `t`.
- `render_issue_human` detail labels (`Status:`/`Type:`/`Priority:`) remain English,
  unchanged by this ADR (separate deferred debt).

## Related

- ADR: [/adr/0004-agent-json-output-contract.md](/adr/0004-agent-json-output-contract.md)
- Issue: [/issues/0006-j5-i18n.md](/issues/0006-j5-i18n.md)
- Issue: [/issues/0007-i18n-interpolation-fix.md](/issues/0007-i18n-interpolation-fix.md)
