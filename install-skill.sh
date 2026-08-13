#!/bin/sh
set -eu

usage() {
  cat <<'EOF'
Usage: install-skill.sh --harness <name> [--scope project|global] [--dir <path>] [--force]
       install-skill.sh -h | --help

Installs the jira-ticket agent-skill thin-pointer stub for one or more agent
harnesses. Each stub only tells the agent to run `jira skill jira-ticket` to
load the full Jira Cloud --json read contract from the CLI; it carries no
--json schema fields, so it can never drift from the contract.

Options:
  --harness <name>   Harness to install for. One of:
                      claude, codex, opencode, pi, copilot, cursor, all
  --scope <value>    project (default) writes under --dir. global writes
                      each harness's real user-level path under $HOME:
                      claude, pi, and codex only. opencode, copilot, and
                      cursor have no standard user-level skills directory
                      and are unsupported under --scope global.
  --dir <path>       Base directory to install into (default: .). Only
                      valid with --scope project.
  --force            Overwrite an existing target file
  -h, --help         Show this help and exit

Also removes a stale pre-rename `jira` stub for the target harness, if found.

When neither --scope nor --dir is given and stdin is a TTY, you are
prompted to choose project or global (default project). A non-TTY run
(e.g. curl | sh) defaults to project with no prompt.
EOF
}

skill_md_body() {
  cat <<'EOF'
---
name: jira-ticket
description: Read Jira Cloud issue data — an issue, your assignments, or a JQL search — as machine-readable JSON from the `jira` CLI, non-interactively without the TUI. Use when an agent or script needs to fetch an issue by key or URL, list the logged-in user's open issues, read the issue for the current git branch, or run a JQL search, and wants structured JSON instead of the interactive terminal UI. Covers `jira get`, `jira current`, `jira mine`, and `jira search` with `--json` — the curated minified schemas, the round-trippable `ref`, and the cache / `--no-comments` / `--refresh` flags. Also covers posting a comment with `jira comment`. Also covers downloading every issue attachment to local disk with `--download-attachments`.
---

# jira-ticket (thin pointer)

The full, authoritative Jira Cloud `--json` read contract is served by the CLI itself.

Run:

    jira skill jira-ticket

and follow its output. It documents the curated minified JSON schemas for
`jira get`, `jira current`, `jira mine`, and `jira search` with `--json`, the
round-trippable `ref`, and the cache / `--no-comments` / `--refresh` flags.
EOF
}

skill_mdc_body() {
  cat <<'EOF'
---
description: Read Jira Cloud issue/assignment/JQL data as JSON via the `jira` CLI (get/current/mine/search --json), non-interactively. Also downloads issue attachments to local disk with --download-attachments. Run `jira skill jira-ticket` for the full contract.
globs:
alwaysApply: false
---

# jira-ticket (thin pointer)

The full Jira Cloud `--json` read contract is served by the CLI. Run `jira skill jira-ticket`
and follow its output — it documents the `--json` schemas for get/current/mine/search,
the round-trippable `ref`, and the cache flags.
EOF
}

write_stub() {
  _target="$1"
  _body_fn="$2"

  if [ -f "${_target}" ] && [ "${_force}" -ne 1 ]; then
    echo "exists, skipping: ${_target} (use --force to overwrite)"
    return 0
  fi

  mkdir -p "$(dirname "${_target}")"
  "${_body_fn}" > "${_target}"
  echo "wrote: ${_target}"
}

remove_legacy_stub() {
  _legacy="$1"

  # The 'jira skill' marker is present in every generation of the thin-pointer
  # stub body, so it distinguishes our stub from an unrelated user file.
  if [ -f "${_legacy}" ] && grep -q 'jira skill' "${_legacy}"; then
    rm -f "${_legacy}"
    rmdir "$(dirname "${_legacy}")" 2>/dev/null || :
    echo "removed legacy stub: ${_legacy}"
  fi
}

unsupported_under_global() {
  echo "global scope is not supported for ${1} (no standard user-level skills directory); install per-project instead" >&2
  return 2
}

install_harness_project() {
  case "$1" in
    claude) remove_legacy_stub "${_dir}/.claude/skills/jira/SKILL.md"; write_stub "${_dir}/.claude/skills/jira-ticket/SKILL.md" skill_md_body ;;
    codex) remove_legacy_stub "${_dir}/.codex/skills/jira/SKILL.md"; write_stub "${_dir}/.codex/skills/jira-ticket/SKILL.md" skill_md_body ;;
    opencode) remove_legacy_stub "${_dir}/.opencode/skills/jira/SKILL.md"; write_stub "${_dir}/.opencode/skills/jira-ticket/SKILL.md" skill_md_body ;;
    pi) remove_legacy_stub "${_dir}/.pi/skills/jira/SKILL.md"; write_stub "${_dir}/.pi/skills/jira-ticket/SKILL.md" skill_md_body ;;
    copilot) remove_legacy_stub "${_dir}/.github/skills/jira/SKILL.md"; write_stub "${_dir}/.github/skills/jira-ticket/SKILL.md" skill_md_body ;;
    cursor) remove_legacy_stub "${_dir}/.cursor/rules/jira.mdc"; write_stub "${_dir}/.cursor/rules/jira-ticket.mdc" skill_mdc_body ;;
  esac
}

install_harness_global() {
  case "$1" in
    claude) remove_legacy_stub "${HOME}/.claude/skills/jira/SKILL.md"; write_stub "${HOME}/.claude/skills/jira-ticket/SKILL.md" skill_md_body ;;
    pi) remove_legacy_stub "${HOME}/.pi/agent/skills/jira/SKILL.md"; write_stub "${HOME}/.pi/agent/skills/jira-ticket/SKILL.md" skill_md_body ;;
    codex) remove_legacy_stub "${HOME}/.codex/skills/jira/SKILL.md"; write_stub "${HOME}/.codex/skills/jira-ticket/SKILL.md" skill_md_body ;;
    opencode|copilot|cursor) unsupported_under_global "$1" ;;
  esac
}

install_harness() {
  _name="$1"
  if [ "${_scope}" = "global" ]; then
    install_harness_global "${_name}"
  else
    install_harness_project "${_name}"
  fi
}

_harness=""
_scope=""
_dir="."
_dir_explicit=0
_force=0

while [ $# -gt 0 ]; do
  case "$1" in
    --harness)
      if [ $# -lt 2 ]; then
        echo "Error: --harness requires a value" >&2
        usage >&2
        exit 2
      fi
      _harness="$2"
      shift 2
      ;;
    --scope)
      if [ $# -lt 2 ]; then
        echo "Error: --scope requires a value" >&2
        usage >&2
        exit 2
      fi
      _scope="$2"
      shift 2
      ;;
    --dir)
      if [ $# -lt 2 ]; then
        echo "Error: --dir requires a value" >&2
        usage >&2
        exit 2
      fi
      _dir="$2"
      _dir_explicit=1
      shift 2
      ;;
    --force)
      _force=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ -n "${_scope}" ]; then
  case "${_scope}" in
    project|global) ;;
    *)
      echo "Error: unknown scope: ${_scope}" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

if [ "${_scope}" = "global" ] && [ "${_dir_explicit}" -eq 1 ]; then
  echo "Error: --dir cannot be combined with --scope global" >&2
  exit 2
fi

if [ -z "${_scope}" ]; then
  if [ "${_dir_explicit}" -eq 0 ] && [ -t 0 ]; then
    printf 'Install scope? [project/global] (default project): '
    read -r _scope_answer
    case "${_scope_answer}" in
      ""|project) _scope="project" ;;
      global) _scope="global" ;;
      *)
        echo "Error: unknown scope: ${_scope_answer}" >&2
        usage >&2
        exit 2
        ;;
    esac
  else
    _scope="project"
  fi
fi

if [ -z "${_harness}" ]; then
  echo "Error: --harness is required" >&2
  usage >&2
  exit 2
fi

case "${_harness}" in
  claude|codex|opencode|pi|copilot|cursor|all) ;;
  *)
    echo "Error: unknown harness: ${_harness}" >&2
    usage >&2
    exit 2
    ;;
esac

_count=0
if [ "${_harness}" = "all" ]; then
  for _h in claude codex opencode pi copilot cursor; do
    if install_harness "${_h}"; then
      _count=$((_count + 1))
    fi
  done
else
  install_harness "${_harness}"
  _count=1
fi

echo "Done: processed ${_count} harness target(s)."
