use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};

use crate::client::{GouqiJiraClient, JiraClient};
use crate::commands::{DEFAULT_SEARCH_LIMIT, MINE_JQL};
use crate::i18n::t;
use crate::models::{Issue, IssueRow};
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;

use super::model::{update, Cmd, Model, Msg, Screen};
use super::view::view;

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

/// Entry point for `jira browse`.
///
/// Checks the TTY guard, then fetches the mine list and enters the raw-mode draw loop.
/// Returns 0 on clean quit (`q` or Ctrl+C) and 1 on the non-TTY guard path or fetch error.
pub async fn browse(
    instance: &Instance,
    cache: &TaskCache<'_>,
    is_tty: bool,
    stderr: &mut impl Write,
) -> i32 {
    use crate::cli::{browse_tty_action, BrowseAction};

    match browse_tty_action(is_tty) {
        BrowseAction::TtyError => {
            writeln!(stderr, "{}", t(TTY_ERROR_KEY)).ok();
            1
        }
        BrowseAction::RunTui => fetch_and_run(instance, cache, stderr).await,
    }
}

pub(crate) async fn fetch_and_run(
    instance: &Instance,
    cache: &TaskCache<'_>,
    stderr: &mut impl Write,
) -> i32 {
    let client = match GouqiJiraClient::new(instance) {
        Ok(c) => c,
        Err(e) => {
            writeln!(stderr, "Error building client: {e}").ok();
            return 1;
        }
    };

    let rows = match client.search(MINE_JQL, DEFAULT_SEARCH_LIMIT).await {
        Ok(result) => result.issues,
        Err(e) => {
            writeln!(stderr, "Error fetching issues: {e}").ok();
            return 1;
        }
    };

    let handle = tokio::runtime::Handle::current();
    run_tui(rows, instance, cache, handle)
}

fn run_tui(
    rows: Vec<IssueRow>,
    instance: &Instance,
    cache: &TaskCache<'_>,
    handle: tokio::runtime::Handle,
) -> i32 {
    let mut stdout = io::stdout();
    if enable_raw_mode().is_err() {
        return 1;
    }
    if execute!(stdout, EnterAlternateScreen).is_err() {
        let _ = disable_raw_mode();
        return 1;
    }

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return 1;
        }
    };

    let model = Model {
        rows,
        selected: 0,
        screen: Screen::List,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: instance.base_url.clone(),
    };
    let exit_code = draw_loop(&mut terminal, model, instance, cache, &handle);

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    exit_code
}

fn map_key_to_msg(key_code: KeyCode, modifiers: KeyModifiers, search_active: bool) -> Option<Msg> {
    if search_active {
        return map_key_in_search_mode(key_code, modifiers);
    }
    map_key_in_normal_mode(key_code, modifiers)
}

fn map_key_in_search_mode(key_code: KeyCode, modifiers: KeyModifiers) -> Option<Msg> {
    match key_code {
        KeyCode::Enter => Some(Msg::SubmitSearch),
        KeyCode::Esc => Some(Msg::CancelSearch),
        KeyCode::Backspace => Some(Msg::SearchBackspace),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char(c) => Some(Msg::SearchInput(c)),
        _ => None,
    }
}

fn map_key_in_normal_mode(key_code: KeyCode, modifiers: KeyModifiers) -> Option<Msg> {
    match key_code {
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::Enter => Some(Msg::Select),
        KeyCode::Esc => Some(Msg::Back),
        KeyCode::Char('b') => Some(Msg::Back),
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Char('/') => Some(Msg::OpenSearch),
        KeyCode::Char('o') => Some(Msg::OpenLink),
        KeyCode::Char('y') => Some(Msg::CopyKey),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        _ => None,
    }
}

fn draw_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    handle: &tokio::runtime::Handle,
) -> i32 {
    loop {
        let _ = terminal.draw(|frame| view(&model, frame));

        match event::read() {
            Ok(Event::Key(key)) => {
                let search_active = model.search.is_some();
                let Some(msg) = map_key_to_msg(key.code, key.modifiers, search_active) else {
                    continue;
                };

                let (next_model, cmds) = update(model, msg);
                model = next_model;

                if cmds.contains(&Cmd::Quit) {
                    return 0;
                }

                for cmd in cmds {
                    model = dispatch_cmd(cmd, model, instance, cache, handle);
                }
            }
            Err(_) => return 1,
            Ok(_) => {}
        }
    }
}

fn dispatch_cmd(
    cmd: Cmd,
    model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    handle: &tokio::runtime::Handle,
) -> Model {
    match cmd {
        Cmd::Quit => model,
        Cmd::OpenUrl(url) => {
            spawn_opener(&url);
            model
        }
        Cmd::CopyToClipboard(key) => {
            copy_to_clipboard(&key);
            model
        }
        Cmd::LoadDetail(key) => {
            let result =
                tokio::task::block_in_place(|| handle.block_on(load_detail(instance, cache, &key)));
            match result {
                Ok(issue) => {
                    let (next, _) = update(model, Msg::DetailLoaded(Box::new(issue)));
                    next
                }
                Err(_) => {
                    let (next, _) = update(model, Msg::Back);
                    next
                }
            }
        }
        Cmd::LoadList(jql) => {
            let result =
                tokio::task::block_in_place(|| handle.block_on(run_search(instance, &jql)));
            match result {
                Ok(rows) => {
                    let (next, _) = update(model, Msg::ListLoaded(rows));
                    next
                }
                Err(e) => {
                    let (next, _) = update(model, Msg::LoadFailed(e.to_string()));
                    next
                }
            }
        }
    }
}

async fn load_detail(instance: &Instance, cache: &TaskCache<'_>, key: &str) -> Result<Issue, i32> {
    let issue_cache = crate::store::cache::IssueCache::new(cache.conn());
    let mut sink: Vec<u8> = Vec::new();
    crate::commands::load_issue(key, instance, &issue_cache, false, &mut sink).await
}

pub(crate) async fn run_search(instance: &Instance, jql: &str) -> anyhow::Result<Vec<IssueRow>> {
    let client = GouqiJiraClient::new(instance)?;
    let result = client.search(jql, DEFAULT_SEARCH_LIMIT).await?;
    Ok(result.issues)
}

fn spawn_opener(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(not(target_os = "macos"))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn copy_to_clipboard(key: &str) {
    use std::io::Write as _;

    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(key.as_bytes());
            }
        }
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let tools = [&["xclip", "-selection", "clipboard"][..], &["wl-copy"][..]];
        for argv in &tools {
            if let Ok(mut child) = std::process::Command::new(argv[0])
                .args(&argv[1..])
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(key.as_bytes());
                }
                return;
            }
        }
    }
}
