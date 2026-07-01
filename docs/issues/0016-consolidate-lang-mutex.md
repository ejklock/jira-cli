---
type: Issue
title: "Consolidate the 4 per-module LANG_MUTEX statics into one crate-wide test lock"
description: Four separate `static LANG_MUTEX` declarations (tests/unit/{render,commands,i18n,tui}.rs) each serialize language-dependent tests within their own module — but because LANGUAGE is a process-global RwLock and cargo runs the whole unit-test binary concurrently, two DISTINCT mutexes do NOT serialize language mutation across modules. Consolidate to a single crate-wide test lock in src/i18n.rs so all language tests serialize on one lock, closing the latent cross-module race.
status: done
tracker:
tags: [i18n, test-infra, refactor, debt]
timestamp: 2026-06-30T00:00:00Z
---

# Consolidate the 4 per-module LANG_MUTEX statics into one crate-wide test lock

## Objective link

Lesson 3331 (every language-dependent test locks a module-level `LANG_MUTEX`, sets the
language explicitly, and resets to `"en"` before returning) and the Reviewer observation
on the i18n-labels slice (run 2591): `render.rs` and `tui.rs` declare **separate**
`LANG_MUTEX` statics, so language mutation across those modules does not serialize —
under `cargo test`'s concurrent execution a pt_BR test in one module can overlap a pt_BR
test in another and leak the global `LANGUAGE` into an assertion. One crate-wide lock
removes the race. Closes the "consolidate the 3 per-module LANG_MUTEX into one crate-wide
lock" deferred test-infra debt.

## Context manifest

- **Read first:** `src/i18n.rs` — `LANGUAGE` (L8, the process-global `RwLock<String>`),
  `set_language` (L58), `current_language` (L64). This is the single home of the language
  global, so the test serialization lock belongs here too.
- The four duplicate declarations (each `static LANG_MUTEX: Mutex<()> = Mutex::new(());`)
  and their `use std::sync::Mutex;`:
  - `tests/unit/render.rs:6` — 10 call sites.
  - `tests/unit/commands.rs:23` — 9 call sites.
  - `tests/unit/i18n.rs:4` — 11 call sites.
  - `tests/unit/tui.rs:9` (L7 `use std::sync::Mutex;`) — 2 call sites.
- Each test module is included via `#[cfg(test)] #[path = "..."] mod tests;` into its
  source file, so all four compile as part of the crate's unit-test binary and can
  reference `crate::i18n::LANG_MUTEX`.

## Approach (decided)

- Add ONE lock to `src/i18n.rs`, gated to the test build, using a fully-qualified type so
  no `use` is added to the non-test build:
  `#[cfg(test)] pub(crate) static LANG_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());`
  Place it near the `LANGUAGE` static (its logical sibling).
- In each of `tests/unit/{render,commands,i18n,tui}.rs`: **delete** the local
  `static LANG_MUTEX: Mutex<()> = Mutex::new(());` and add `use crate::i18n::LANG_MUTEX;`.
  Keep **every** `let _lock = LANG_MUTEX.lock().unwrap();` call site byte-identical.
- Remove each now-orphaned `use std::sync::Mutex;` **only if** `Mutex` is no longer
  referenced elsewhere in that file (clippy `--all-targets -D warnings` is the backstop —
  an unused import must not remain).

## Vertical Demo

- **Given** the consolidation has landed,
  **When** I run `docker compose run --rm dev cargo test`,
  **Then** all language-dependent tests across render/commands/i18n/tui pass, now
  serialized on the single `crate::i18n::LANG_MUTEX`, and
  `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check` are clean.

## Acceptance

| AC | Kind | Condition | Instrument (verify_by) |
|---|---|---|---|
| AC1 | constraint | Exactly ONE `static LANG_MUTEX` exists in the crate — `#[cfg(test)] pub(crate)` in `src/i18n.rs`; the four per-module declarations are gone; every call site references the shared lock via `use crate::i18n::LANG_MUTEX;` | inspection |
| AC2 | behavior | The full test suite passes; every language test still acquires the lock, sets the language, and resets to `"en"` (no call-site logic changed) | command (`cargo test`) |
| AC3 | constraint | Language mutation across ALL four modules now serializes on the single lock (the race is closed); no `let _lock = LANG_MUTEX.lock()` call site was altered beyond the declaration source | inspection |
| AC4 | constraint | clippy `--all-targets -- -D warnings` clean (no orphaned `use std::sync::Mutex;`), `cargo fmt --check` clean, `cargo test --test comment_policy` clean | command (clippy + fmt + comment_policy) |
| AC5 | constraint | No superfluous comments / banners / commented-out code; only non-obvious why-comments; the added static carries at most a one-line why-comment | inspection |

## Out of scope

- Introducing a `with_language(lang, || …)` guard helper that wraps set/reset around a
  closure — a larger churn touching all ~32 call sites; deferred (this slice is the
  minimal race fix, not an ergonomics redesign).
- Any change to `set_language`/`current_language`/`t`/`tf` or the `LANGUAGE` global itself.
- Any production (non-test) behavior change.

## blocked_by

(none — isolated to test infra + one `#[cfg(test)]` static in i18n.rs)
