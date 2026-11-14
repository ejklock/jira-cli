# jira-cli-agent

Gives AI coding agents your Jira Cloud issues through the
[`jira` CLI](https://github.com/ejklock/jira-cli):

- **OpenCode**: registers the `jira mcp` server (named `jira-cli`) and the
  `jira-ticket` skill.
- **Pi**: adds the `jira-ticket` skill, which drives `jira` with `--json`.

Unofficial; not affiliated with Atlassian.

## Install

Install `jira` 0.2.0 or later, with the `mcp` subcommand, on your `PATH` first.

OpenCode, in `opencode.json`:

```json
{ "plugin": ["jira-cli-agent"] }
```

Pi:

```sh
pi install npm:jira-cli-agent
```

A key in your own `mcp` config named `jira-cli` wins over the one this
plugin registers.

Claude Code and Codex install the same tools from the repository's plugin
marketplace; see the [project README](https://github.com/ejklock/jira-cli#plugins).

## License

MIT
