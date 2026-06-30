mod agent_json;
mod cli;
mod client;
mod commands;
mod config;
mod i18n;
mod models;
mod render;
mod store;
mod timing;

use clap::{CommandFactory, Parser};
use cli::{bare_no_command_action, BareNoCommandAction, Cli, Command};
use commands::{
    pick_instance, setup_add, setup_list, setup_remove, setup_test, GetOpts, SetupAddFields,
};
use std::io::IsTerminal;
use std::process;

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
    }
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
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    let mut err_buf = std::io::stderr();
    let idx = match pick_instance(&instances, args.display.instance.as_deref(), &mut err_buf) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let inst = instances[idx].clone();
    let cache = store::cache::TaskCache::new(store.conn());
    commands::get_core(
        &args.ref_,
        &inst,
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

async fn dispatch_current(args: cli::DisplayArgs) -> i32 {
    let branch = current_git_branch();
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    let mut err_buf = std::io::stderr();
    let idx = match pick_instance(&instances, args.instance.as_deref(), &mut err_buf) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let inst = instances[idx].clone();
    let cache = store::cache::TaskCache::new(store.conn());
    commands::current_core(
        branch.as_deref(),
        &inst,
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

async fn dispatch_search(args: cli::SearchArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    let mut err_buf = std::io::stderr();
    let idx = match pick_instance(&instances, args.instance.as_deref(), &mut err_buf) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let inst = instances[idx].clone();
    commands::search_core(
        args.jql.as_deref(),
        &inst,
        args.json,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
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
