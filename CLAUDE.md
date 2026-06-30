# jira-cli — Project Guide

Jira Cloud read/browse CLI. A Rust single binary `jira`, built and distributed via
Docker. The codebase is a **fork of `active-collab-cli`** with the API/auth/domain
layer swapped to Jira Cloud. See the docs trail below.

## Docs index

Living documentation lives in [`docs/`](docs/index.md). Start at:

- [Constitution](docs/constitution.md) — scope, data model, non-negotiables.
- [PRD 0001](docs/prd/0001-jira-cloud-read-cli.md) — the v1 read-CLI capability.
- [ADR 0001](docs/adr/0001-fork-active-collab-cli-swap-api.md) — fork the AC base.
- [ADR 0002](docs/adr/0002-jira-cloud-only-basic-auth.md) — Cloud-only + Basic auth.
- [Architecture](docs/architecture.md) — module + data-flow diagrams.
- [Issues](docs/issues/index.md) — slices J0–J5.

## Build & run commands — HARD RULES

There is **no local Rust toolchain**. The Cargo crate is at the repo root; the
compose file (`docker-compose.yml`) is also at the repo root, and its `dev`
service mounts `./` with `working_dir` `/app`. Therefore:

1. **Run every build/test/lint command from the repo root, bare:**
   - `docker compose run --rm dev cargo build`
   - `docker compose run --rm dev cargo test`
   - `docker compose run --rm dev cargo test --test comment_policy` (comment-policy gate: no banners, no commented-out code; doc comments and non-obvious why-comments are allowed)
   - `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
     (use `--all-targets` to match CI: without it clippy does **not** lint the
     `#[path]`-included `#[cfg(test)]` modules.)
   - `docker compose run --rm dev cargo fmt --check`
   - `docker compose build` / `docker compose run --rm build` (release)
2. **NEVER prefix a command with `cd`.** The shell's working directory is already
   the repo root and persists between commands. `cd` (especially combined with a
   pipe or redirect) trips Claude Code's path-resolution guard and forces a manual
   approval prompt on every call.
3. **NEVER use absolute paths.** Use the bare command above; compose auto-discovers
   the root `docker-compose.yml`. If you must point at a file, use `./docker-compose.yml`.
4. **Do not append `2>/dev/null`, `| head`, `&& echo …`, or extra chaining** to a
   command unless required — these can also trip the bypass guard.
5. **Cargo fetches from crates.io / static.crates.io**, outside the default command
   sandbox allowlist. The FIRST build/test (and image pulls) need the network, so
   run those specific commands with the sandbox disabled — a legitimate registry fetch.

## Living Docs
enforcement: strict   # strict | guided | lite
onboarded: 2026-06-29

## Maintenance rule

Any structural change updates its doc **and its Mermaid diagram** in the same
change ([architecture](docs/architecture.md), the relevant ADR/BDR, and the
directory `index.md`). No orphan docs, no stale diagrams.

## Conventions

- Docs language: English. User-facing chat: Brazilian Portuguese.
- No AI attribution in commits or docs.
- The domain core (rendering, command resolution, `agent_json` shaping) is pure
  (no network/filesystem) and unit-tested.
