#!/bin/sh
# The plugin and npm package copies of the skill stub must equal what install-skill.sh writes,
# because install-skill.sh runs as `curl | sh` and cannot read a repo file.
# The plugin, marketplace and npm package names and versions must equal Cargo.toml's.
# CHECK_STUBS_ROOT overrides the checked tree (used to test the checks on a copy).
set -eu

root="${CHECK_STUBS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
plugin_dir="${root}/plugins/jira-cli"
plugin_skill="${plugin_dir}/skills/jira-ticket/SKILL.md"
package_dir="${root}/packages/jira-cli-agent"
package_skill="${package_dir}/skills/jira-ticket/SKILL.md"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT INT TERM

status=0

if ! sh "${root}/install-skill.sh" --harness claude --scope project --dir "${tmp}" >/dev/null; then
  echo "install-skill.sh failed; cannot compare the skill stub" >&2
  exit 1
fi

if ! diff -u "${tmp}/.claude/skills/jira-ticket/SKILL.md" "${plugin_skill}"; then
  echo "plugin SKILL.md differs from the install-skill.sh stub" >&2
  status=1
fi

if ! diff -u "${tmp}/.claude/skills/jira-ticket/SKILL.md" "${package_skill}"; then
  echo "packages/jira-cli-agent SKILL.md differs from the install-skill.sh stub" >&2
  status=1
fi

version="$(sed -n 's/^version *= *"\(.*\)"$/\1/p' "${root}/Cargo.toml" | head -n 1)"
name="jira-cli"

# expect FILE JQ_FILTER WANT: print the file and field when the value differs.
expect() {
  got="$(jq -r "$2" "${root}/$1")"
  if [ "${got}" != "$3" ]; then
    echo "$1: $2 is '${got}', expected '$3'" >&2
    status=1
  fi
}

for manifest in plugins/jira-cli/.claude-plugin/plugin.json plugins/jira-cli/plugin.json; do
  expect "${manifest}" '.name' "${name}"
  expect "${manifest}" '.version' "${version}"
done

expect packages/jira-cli-agent/package.json '.name' "jira-cli-agent"
expect packages/jira-cli-agent/package.json '.version' "${version}"

# expect_entries FILE: every marketplace entry carries the plugin name and no
# version, and one entry's source points at the plugin directory.
expect_entries() {
  count="$(jq -r '.plugins | length' "${root}/$1")"
  i=0
  while [ "${i}" -lt "${count}" ]; do
    expect "$1" ".plugins[${i}].name" "${name}"
    expect "$1" ".plugins[${i}].version // \"none\"" "none"
    i=$((i + 1))
  done
  expect "$1" '[.plugins[] | (.source | if type == "object" then .path else . end) | select(. == "./plugins/jira-cli")] | length | if . > 0 then "found" else "missing" end' "found"
}
expect_entries .claude-plugin/marketplace.json
expect_entries .agents/plugins/marketplace.json

if [ "${status}" -ne 0 ]; then
  exit 1
fi
echo "plugin and jira-cli-agent SKILL.md match the install-skill.sh stub; plugin names and versions match Cargo.toml"
