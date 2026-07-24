mod agent_json;
mod cli;
mod client;
mod commands;
mod config;
mod download;
mod i18n;
mod models;
mod render;
mod skill;
mod store;
mod timing;
mod tui;

#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;

use clap::{CommandFactory, Parser};
use cli::{
    bare_no_command_action, command_surface, extract_issue_key, BareNoCommandAction, Cli, Command,
    Surface,
};
use client::{ClientError, GouqiJiraClient, JiraClient};
use commands::{
    parse_issue_ref, pick_instance, reauth_message, setup_add, setup_list, setup_remove,
    setup_test, CommentBody, GetOpts, SetupAddFields,
};
use std::io::IsTerminal;
use std::process;
use store::instances::Instance;

#[tokio::main]
async fn main() {
    let code = run(std::env::args().skip(1).collect()).await;
    process::exit(code);
}

async fn run(raw_argv: Vec<String>) -> i32 {
    let branch = current_git_branch();
    let argv = cli::normalize_argv(&raw_argv, branch.as_deref());

    let cli_result = Cli::try_parse_from(std::iter::once("jira".to_owned()).chain(argv));

    let cli = match cli_result {
        Ok(c) => c,
        Err(e) => {
            e.print().ok();
            return if e.exit_code() == 0 { 0 } else { e.exit_code() };
        }
    };

    let Some(command) = cli.command else {
        let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
        return match bare_no_command_action(is_tty) {
            BareNoCommandAction::RunMine => {
                init_language();
                dispatch_mine(cli::MineArgs {
                    instance: None,
                    json: false,
                    limit: None,
                })
                .await
            }
            BareNoCommandAction::HelpExit2 => {
                let mut help_cli = Cli::command();
                help_cli.print_help().ok();
                eprintln!();
                2
            }
        };
    };

    init_language();
    dispatch(command).await
}

fn init_language() {
    let env_value = std::env::var("JIRA_CLI_LANG").ok();

    let db_value: Option<String> = (|| -> Option<String> {
        let config = config::load();
        let store = store::Store::open(&config).ok()?;
        store::settings::SettingsRepository::new(store.conn())
            .get("language", None)
            .ok()
            .flatten()
    })();

    let lang = i18n::resolve_language(env_value.as_deref(), db_value.as_deref());
    i18n::set_language(&lang);
}

async fn dispatch(command: Command) -> i32 {
    match command {
        Command::Setup(opts) => dispatch_setup(opts.subcommand).await,
        Command::Get(args) => dispatch_get(args).await,
        Command::Current(args) => dispatch_current(args).await,
        Command::Mine(args) => dispatch_mine(args).await,
        Command::Search(args) => dispatch_search(args).await,
        Command::Browse(args) => dispatch_browse(args).await,
        Command::Comment(args) => dispatch_comment(args).await,
        Command::Skill(args) => dispatch_skill(args),
    }
}

fn dispatch_skill(args: cli::SkillArgs) -> i32 {
    skill::skill_output(
        args.name.as_deref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

async fn dispatch_setup(cmd: cli::SetupCmd) -> i32 {
    match cmd {
        cli::SetupCmd::Add(args) => dispatch_setup_add(args).await,
        cli::SetupCmd::List => dispatch_setup_list(),
        cli::SetupCmd::Remove(args) => dispatch_setup_remove(args),
        cli::SetupCmd::Test(args) => dispatch_setup_test(args).await,
        cli::SetupCmd::Language(args) => dispatch_setup_language(args),
    }
}

fn dispatch_setup_language(args: cli::LanguageArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::settings::SettingsRepository::new(store.conn());
    commands::setup_language(
        &repo,
        args.code.as_deref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

fn open_store() -> Option<store::Store> {
    let config = config::load();
    match store::Store::open(&config) {
        Ok(s) => Some(s),
        Err(e) => {
            render::print_error(&format!("Error opening database: {e}"));
            None
        }
    }
}

struct ResolvedInstance {
    store: store::Store,
    instance: Instance,
}

fn resolve_single_instance(instance_filter: Option<&str>) -> Result<ResolvedInstance, i32> {
    let store = open_store().ok_or(1)?;
    let instances = {
        let repo = store::instances::InstanceRepository::new(store.conn());
        repo.load_all().map_err(|e| {
            render::print_error(&format!("Error loading instances: {e}"));
            1
        })?
    };
    let mut err_buf = std::io::stderr();
    let idx = pick_instance(&instances, instance_filter, &mut err_buf)?;
    let instance = instances[idx].clone();
    Ok(ResolvedInstance { store, instance })
}

fn dispatch_setup_list() -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_list(&repo, &mut std::io::stdout())
}

fn dispatch_setup_remove(args: cli::RemoveArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let cache = store::cache::TaskCache::new(store.conn());
    setup_remove(
        &repo,
        &cache,
        &args.name,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

async fn dispatch_setup_test(args: cli::TestArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_test(
        &repo,
        args.name.as_deref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_setup_add(args: cli::SetupAddArgs) -> i32 {
    let interactive = stdin_is_tty();

    let name = resolve_field(args.name, "Instance name", interactive);
    let url = resolve_field(args.url, "Base URL (https://...)", interactive);
    let email = resolve_field(args.email, "Email", interactive);

    let token = if interactive {
        read_password_interactive()
    } else {
        read_password_stdin()
    };

    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_add(
        SetupAddFields { name, url, email },
        token,
        &repo,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

fn read_password_interactive() -> Option<String> {
    print!("API token (input hidden): ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    let trimmed = line
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn read_password_stdin() -> Option<String> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    let trimmed = line
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn resolve_field(value: Option<String>, label: &str, interactive: bool) -> Option<String> {
    if let Some(v) = value {
        if !v.is_empty() {
            return Some(v);
        }
    }
    if !interactive {
        return None;
    }
    loop {
        print!("{label}: ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        let val = input.trim().to_owned();
        if !val.is_empty() {
            return Some(val);
        }
        eprintln!("{label} cannot be empty.");
    }
}

fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

async fn dispatch_get(args: cli::GetArgs) -> i32 {
    if args.display.download_attachments {
        return match parse_issue_ref(&args.ref_) {
            Some(key) => dispatch_download_attachments(&key, &args.display).await,
            None => {
                eprintln!(
                    "Error: '{}' is not a valid issue key or Jira browse URL.",
                    args.ref_
                );
                2
            }
        };
    }

    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if command_surface(is_tty, args.display.json) == Surface::Interactive {
        if let Some(key) = parse_issue_ref(&args.ref_) {
            return dispatch_get_interactive(args.display.instance.as_deref(), key).await;
        }
    }

    let ResolvedInstance { store, instance } =
        match resolve_single_instance(args.display.instance.as_deref()) {
            Ok(r) => r,
            Err(code) => return code,
        };
    let cache = store::cache::TaskCache::new(store.conn());
    commands::get_core(
        &args.ref_,
        &instance,
        &cache,
        GetOpts {
            json: args.display.json,
            no_comments: args.display.no_comments,
            refresh: args.display.refresh,
        },
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// Interactive surface for `get <ref>` (ADR 0025, BDR 0016 S7): resolves the
/// instance the same way `browse` does, then opens the TUI seeded directly
/// on the resolved issue's detail. Reached only when `ref_` parses to a
/// valid issue key or browse URL — an unparseable ref falls through to
/// `get_core` for its existing "not a valid issue key" error (exit 2),
/// unchanged.
async fn dispatch_get_interactive(instance_filter: Option<&str>, key: String) -> i32 {
    let ResolvedInstance { store, instance } = match resolve_single_instance(instance_filter) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let cache = store::cache::TaskCache::new(store.conn());
    let is_tty = std::io::stdout().is_terminal();
    tui::browse_seeded(
        &instance,
        &cache,
        is_tty,
        tui::TuiSeed::Detail(key),
        &mut std::io::stderr(),
    )
    .await
}

/// Resolve the issue key `jira current --download-attachments` operates on
/// from the current git branch, mirroring `current_core`'s two distinct
/// error messages (no branch vs. no key in the branch name).
fn resolve_current_branch_key(branch: Option<&str>) -> Result<String, i32> {
    let branch_name = branch.ok_or_else(|| {
        eprintln!("Error: not in a git repository / no current branch.");
        2
    })?;
    extract_issue_key(branch_name).ok_or_else(|| {
        eprintln!("Error: no issue key in branch '{branch_name}'.");
        2
    })
}

/// Shared `--download-attachments` implementation for `get` and `current`
/// (ADR 0029 §2, BDR 0020 S4-S7): fetches `key`, downloads every attachment
/// via the D2a seam to `display.download_dir` (or the default config
/// downloads dir), and reports the saved paths. Download-only — never also
/// renders the full issue.
async fn dispatch_download_attachments(key: &str, display: &cli::DisplayArgs) -> i32 {
    let ResolvedInstance { instance, .. } =
        match resolve_single_instance(display.instance.as_deref()) {
            Ok(r) => r,
            Err(code) => return code,
        };
    let client = match GouqiJiraClient::new(&instance) {
        Ok(c) => c,
        Err(e) => {
            render::print_error(&format!("Error building Jira client: {e}"));
            return 1;
        }
    };
    let issue = match client.get_issue(key).await {
        Ok(i) => i,
        Err(ClientError::Unauthorized { instance }) => {
            eprintln!("{}", reauth_message(&instance));
            return 1;
        }
        Err(ClientError::Other(e)) => {
            render::print_error(&format!("Error fetching issue '{key}': {e}"));
            return 1;
        }
    };

    let dir = display
        .download_dir
        .clone()
        .unwrap_or_else(|| download::download_dir_for(&config::jira_config_dir(), &issue.key));

    match download::download_all(&client, &issue, &dir).await {
        Ok(saved) => {
            print_download_result(&issue.key, &saved, display.json);
            0
        }
        Err(e) => {
            render::print_error(&format!("Error downloading attachments: {e}"));
            1
        }
    }
}

fn print_download_result(issue_key: &str, saved: &[download::SavedAttachment], json: bool) {
    if json {
        println!("{}", download::saved_to_json(issue_key, saved));
        return;
    }
    if saved.is_empty() {
        println!("{}", download::no_attachments_message(issue_key));
    } else {
        println!("{}", download::format_saved_human(saved));
    }
}

/// Interactive surface for `current` (ADR 0025, BDR 0016 S9): resolves the
/// issue key from the git branch with the same `extract_issue_key` seam
/// `current_core` uses, so interactive and agent mode never diverge on which
/// issue they open, then reuses `dispatch_get_interactive`'s Detail seed
/// (S3). No resolvable branch key falls through to `current_core` in both
/// modes, unchanged.
async fn dispatch_current(args: cli::DisplayArgs) -> i32 {
    let branch = current_git_branch();

    if args.download_attachments {
        return match resolve_current_branch_key(branch.as_deref()) {
            Ok(key) => dispatch_download_attachments(&key, &args).await,
            Err(code) => code,
        };
    }

    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if command_surface(is_tty, args.json) == Surface::Interactive {
        if let Some(key) = branch.as_deref().and_then(extract_issue_key) {
            return dispatch_get_interactive(args.instance.as_deref(), key).await;
        }
    }

    let ResolvedInstance { store, instance } =
        match resolve_single_instance(args.instance.as_deref()) {
            Ok(r) => r,
            Err(code) => return code,
        };
    let cache = store::cache::TaskCache::new(store.conn());
    commands::current_core(
        branch.as_deref(),
        &instance,
        &cache,
        GetOpts {
            json: args.json,
            no_comments: args.no_comments,
            refresh: args.refresh,
        },
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_mine(args: cli::MineArgs) -> i32 {
    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if command_surface(is_tty, args.json) == Surface::Interactive {
        return dispatch_mine_interactive(args.instance.as_deref()).await;
    }

    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    commands::mine_core(
        &repo,
        args.instance.as_deref(),
        args.json,
        args.limit,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// Interactive surface for `mine`/bare `jira` (ADR 0025, BDR 0016 S1/S2):
/// resolves the instance the same way `browse` does, then opens the TUI
/// seeded on the mine list.
async fn dispatch_mine_interactive(instance_filter: Option<&str>) -> i32 {
    let ResolvedInstance { store, instance } = match resolve_single_instance(instance_filter) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let cache = store::cache::TaskCache::new(store.conn());
    let is_tty = std::io::stdout().is_terminal();
    tui::browse_seeded(
        &instance,
        &cache,
        is_tty,
        tui::TuiSeed::Mine,
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_search(args: cli::SearchArgs) -> i32 {
    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    let trimmed_jql = args.jql.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if command_surface(is_tty, args.json) == Surface::Interactive {
        if let Some(jql) = trimmed_jql {
            return dispatch_search_interactive(args.instance.as_deref(), jql).await;
        }
    }

    let ResolvedInstance { store: _, instance } =
        match resolve_single_instance(args.instance.as_deref()) {
            Ok(r) => r,
            Err(code) => return code,
        };
    commands::search_core(
        args.jql.as_deref(),
        &instance,
        args.json,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// Interactive surface for `search <jql>` (ADR 0025, BDR 0016 S5): resolves
/// the instance the same way `browse` does, then opens the TUI seeded on
/// that JQL's result list. Reached only when `jql` is present and non-blank
/// — a blank/missing JQL falls through to `search_core` for its existing
/// "search requires a JQL query" error (exit 2), unchanged.
async fn dispatch_search_interactive(instance_filter: Option<&str>, jql: &str) -> i32 {
    let ResolvedInstance { store, instance } = match resolve_single_instance(instance_filter) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let cache = store::cache::TaskCache::new(store.conn());
    let is_tty = std::io::stdout().is_terminal();
    tui::browse_seeded(
        &instance,
        &cache,
        is_tty,
        tui::TuiSeed::Search(jql.to_owned()),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_comment(args: cli::CommentArgs) -> i32 {
    let ResolvedInstance { store: _, instance } = match resolve_single_instance(None) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let branch = current_git_branch();
    let body = resolve_comment_body_source(args.message);
    commands::comment_core(
        args.issue_key.as_deref(),
        branch.as_deref(),
        body,
        &instance,
        args.json,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// `-m` wins; otherwise read stdin to EOF only when it is not a TTY (ADR 0023
/// §2) — an interactive invocation without `-m` must fail fast rather than
/// block waiting for input the user never intended to pipe.
fn resolve_comment_body_source(message: Option<String>) -> CommentBody {
    if let Some(m) = message {
        return CommentBody::Flag(m);
    }
    if std::io::stdin().is_terminal() {
        return CommentBody::None;
    }
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok();
    CommentBody::Piped(buf)
}

async fn dispatch_browse(args: cli::BrowseArgs) -> i32 {
    let ResolvedInstance { store, instance } =
        match resolve_single_instance(args.instance.as_deref()) {
            Ok(r) => r,
            Err(code) => return code,
        };
    let cache = store::cache::TaskCache::new(store.conn());
    let is_tty = std::io::stdout().is_terminal();
    tui::browse(&instance, &cache, is_tty, &mut std::io::stderr()).await
}

fn current_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}
