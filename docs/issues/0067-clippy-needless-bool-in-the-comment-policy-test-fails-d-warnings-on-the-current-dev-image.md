---
type: Issue
title: clippy needless_bool in the comment-policy test fails -D warnings on the current dev image
description: tests/comment_policy.rs:107 trips clippy::needless_bool with the dev image's newer clippy, so clippy --all-targets -D warnings exits 101 on an unchanged file.
status: open
timestamp: 2026-11-14T16:31:24Z
---

## 0067. clippy needless_bool in the comment-policy test fails -D warnings on the current dev image

### Problem

`docker compose run --rm dev cargo clippy --all-targets -- -D warnings` exits 101 on `tests/comment_policy.rs:107` (`clippy::needless_bool`) with the dev image's current clippy. Found on 2026-11-14 while proving [issue 0063](/issues/0063-jira-mcp-serve-the-cli-as-mcp-tools-over-stdio.md); the file is unchanged on that branch (`git diff --quiet main -- tests/comment_policy.rs`), so the finding predates it. The dev image floats on `rust:1-slim` and the crate pins no toolchain, so a newer clippy can turn CI red with no code change.

### Scope

Fix the `needless_bool` expression without changing what the comment-policy test asserts. Whether to pin the toolchain is a separate decision for the owner.

### Acceptance

- AC1: `docker compose run --rm dev cargo clippy --all-targets -- -D warnings` exits 0. (`verify_by: command`)
- AC2: `docker compose run --rm dev cargo test --test comment_policy` passes with the same test count. (`verify_by: test`)
