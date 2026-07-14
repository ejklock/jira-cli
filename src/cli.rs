use clap::{Args, Parser, Subcommand};
use regex::Regex;
use std::sync::OnceLock;

pub const KNOWN_COMMANDS: [&str; 8] = [
    "setup", "get", "current", "mine", "list", "search", "browse", "comment",
];

/// Fetch Jira issues from one or more configured instances.
#[derive(Parser, Debug)]
#[command(name = "jira", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Manage instance configuration.
    Setup(SetupOpts),
    /// Fetch and display an issue.
    Get(GetArgs),
    /// Fetch the issue from the current git branch.
    Current(DisplayArgs),
    /// List open issues assigned to you.
    #[command(alias = "list")]
    Mine(MineArgs),
    /// Search for issues.
    Search(SearchArgs),
    /// Open the interactive TUI browser.
    Browse(BrowseArgs),
    /// Post a comment to an issue (non-interactive, one-shot).
    Comment(CommentArgs),
}

/// Wrapper that holds the setup subcommand.
#[derive(Args, Debug)]
pub struct SetupOpts {
    #[command(subcommand)]
    pub subcommand: SetupCmd,
}

#[derive(Subcommand, Debug)]
pub enum SetupCmd {
    /// Register a Jira instance.
    Add(SetupAddArgs),
    /// List configured instances.
    List,
    /// Remove an instance.
    Remove(RemoveArgs),
    /// Test connectivity.
    Test(TestArgs),
    /// Show or set the display language.
    Language(LanguageArgs),
}

#[derive(Args, Debug)]
pub struct LanguageArgs {
    /// Language code to set (en, pt-BR). Omit to show current.
    #[arg(value_name = "CODE")]
    pub code: Option<String>,
}

#[derive(Args, Debug)]
pub struct SetupAddArgs {
    /// Unique name (prompted if omitted, interactive).
    #[arg(long)]
    pub name: Option<String>,
    /// Base URL, e.g. https://yourorg.atlassian.net.
    #[arg(long)]
    pub url: Option<String>,
    /// Email for Basic auth.
    #[arg(long)]
    pub email: Option<String>,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Name of the instance to remove.
    #[arg(long, required = true)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Test only this instance.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct GetArgs {
    /// Issue key or URL (e.g. PROJ-123 or https://org.atlassian.net/browse/PROJ-123).
    pub ref_: String,
    #[command(flatten)]
    pub display: DisplayArgs,
}

#[derive(Args, Debug)]
pub struct DisplayArgs {
    /// Force a named instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print raw issue JSON.
    #[arg(long)]
    pub json: bool,
    /// Ignore cache and re-fetch.
    #[arg(long)]
    pub refresh: bool,
    /// Omit the comments section.
    #[arg(long)]
    pub no_comments: bool,
}

#[derive(Args, Debug)]
pub struct MineArgs {
    /// Limit to this instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print curated minified JSON for agent/LLM consumption.
    #[arg(long)]
    pub json: bool,
    /// Cap the number of results.
    #[arg(long)]
    pub limit: Option<u64>,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// JQL query string.
    pub jql: Option<String>,
    /// Force a named instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print curated minified JSON for agent/LLM consumption.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct BrowseArgs {
    /// Limit to this instance.
    #[arg(long)]
    pub instance: Option<String>,
}

#[derive(Args, Debug)]
pub struct CommentArgs {
    /// Issue key (e.g. PROJ-123). Resolved from the current git branch when omitted.
    pub issue_key: Option<String>,
    /// Comment body text. Falls back to piped stdin (multi-line, verbatim) when omitted.
    #[arg(short = 'm', long = "message")]
    pub message: Option<String>,
    /// Print curated minified JSON write result for agent/LLM consumption.
    #[arg(long)]
    pub json: bool,
}

/// Mirror of Python `_normalize_argv`.
///
/// A first arg that is not a known command and not a `-` flag gets `"get"` prepended.
/// Empty argv passes through unchanged.
///
/// `current_branch` is injected so the function remains pure and unit-testable.
pub fn normalize_argv(argv: &[String], current_branch: Option<&str>) -> Vec<String> {
    if let Some(first) = argv.first() {
        if !first.starts_with('-') && !KNOWN_COMMANDS.contains(&first.as_str()) {
            let mut out = vec!["get".to_owned()];
            out.extend_from_slice(argv);
            return out;
        }
        return argv.to_vec();
    }

    if let Some(branch) = current_branch {
        if branch_matches_issue_pattern(branch) {
            return vec!["current".to_owned()];
        }
    }

    argv.to_vec()
}

/// Returns the first Jira issue key found in `branch`, or `None` if no key is present.
///
/// The key regex `[A-Z][A-Z0-9]+-\d+` matches regardless of any branch prefix
/// (`feature/PROJ-123-foo`, `bugfix/ABC-9`, bare `PROJ-123`). First match wins.
pub fn extract_issue_key(branch: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[A-Z][A-Z0-9]+-\d+").expect("valid regex"));
    re.find(branch).map(|m| m.as_str().to_owned())
}

fn branch_matches_issue_pattern(branch: &str) -> bool {
    extract_issue_key(branch).is_some()
}

/// Routing decision when `jira` is invoked with no subcommand.
#[derive(Debug, PartialEq)]
pub enum BareNoCommandAction {
    RunMine,
    HelpExit2,
}

/// Pure routing: in a full TTY session launch the personal issue view;
/// in a pipe or script fall back to help output so scripts are unaffected.
pub fn bare_no_command_action(is_tty: bool) -> BareNoCommandAction {
    if is_tty {
        BareNoCommandAction::RunMine
    } else {
        BareNoCommandAction::HelpExit2
    }
}

/// Routing decision for `jira browse`.
#[derive(Debug, PartialEq)]
pub enum BrowseAction {
    RunTui,
    TtyError,
}

/// Pure routing: launch the TUI when stdout is a TTY; emit an error and exit
/// non-zero when invoked from a pipe or script (no terminal available).
pub fn browse_tty_action(is_tty: bool) -> BrowseAction {
    if is_tty {
        BrowseAction::RunTui
    } else {
        BrowseAction::TtyError
    }
}

/// Interactive-vs-agent routing surface for TTY-default read commands (ADR
/// 0025, BDR 0016): `Interactive` opens the seeded browse TUI; `Agent` prints
/// the existing human/agent_json output unchanged.
#[derive(Debug, PartialEq)]
pub enum Surface {
    Interactive,
    Agent,
}

/// Pure routing: a full TTY session (stdout AND stdin) without `--json` gets
/// the interactive surface; `--json` or a non-TTY end always gets the agent
/// surface (BDR 0016 S1/S3/S4).
pub fn command_surface(is_tty: bool, json: bool) -> Surface {
    if is_tty && !json {
        Surface::Interactive
    } else {
        Surface::Agent
    }
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
