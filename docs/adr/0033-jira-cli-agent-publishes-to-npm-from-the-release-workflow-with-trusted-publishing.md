---
type: ADR
title: jira-cli-agent publishes to npm from the release workflow with trusted publishing
description: Each v* tag publishes jira-cli-agent to npm from release.yml through OIDC trusted publishing, failing when the package version differs from the tag.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-25T10:32:53Z
---

# 0033. jira-cli-agent publishes to npm from the release workflow with trusted publishing

## Context

[ADR 0032](0032-agent-plugins-for-four-harnesses-ship-from-this-repository.md) left `npm publish` of `jira-cli-agent` to the owner, by hand with 2FA, outside the repository's automation. The GitHub release and npm then drifted: on 2026-11-25 npm still served `jira-cli-agent` 0.2.0 while the latest GitHub release was v0.2.1. The owner asked on 2026-11-25 that each release also publish the package to npm.

## Decision

- The `npm-publish` job of `.github/workflows/release.yml` publishes `jira-cli-agent` on each `v*` tag, after the `release` job succeeds.
- It authenticates with npm trusted publishing (OIDC): the job has `id-token: write`, `actions/setup-node` on Node 24 (npm 11.5.1 or later) with `registry-url`, and runs `npm publish --provenance --access public` from `packages/jira-cli-agent`. No npm token is stored in the repository.
- The job fails before publishing when `packages/jira-cli-agent/package.json` `version`, prefixed with `v`, differs from the tag.
- The package's trusted publisher is configured once on npmjs.com by the owner: GitHub Actions, repository `ejklock/jira-cli`, workflow `release.yml`, no environment.

This replaces ADR 0032's consequence "`npm publish` runs with the owner's 2FA, outside this repository's automation"; the rest of ADR 0032 stands.

## Alternatives

- Keep the manual `npm publish`: the package lags the release whenever the step is forgotten. Not taken.
- An `NPM_TOKEN` secret with a granular automation token: a long-lived credential to rotate and guard. Not taken.

## Consequences

- A release tag whose package version is wrong fails the `npm-publish` job after the GitHub release is already published; the fix is a corrected tag, not a rerun.
- Rerunning the job for a version already on npm fails, since npm refuses to publish over an existing version.
- Until the trusted publisher is configured on npmjs.com, the job fails at `npm publish`.
