use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use tokio::sync::mpsc;

use crate::client::{GouqiJiraClient, JiraClient};
use crate::commands::{DEFAULT_SEARCH_LIMIT, MINE_JQL};
use crate::i18n::t;
use crate::models::{Issue, IssueRow, SearchResult};
use crate::store::cache::{IssueCache, TaskCache};
use crate::store::instances::Instance;

use super::model::{update, Cmd, Model, Msg, Screen};
use super::view::view;

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

/// Entry point for `jira browse`.
///
/// Checks the TTY guard, then fetches the mine list and enters the raw-mode async
/// event loop. Returns 0 on clean quit (`q` or Ctrl+C) and 1 on the non-TTY guard
/// path or fetch error.
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

    let result = match client.search(MINE_JQL, DEFAULT_SEARCH_LIMIT).await {
        Ok(result) => result,
        Err(e) => {
            writeln!(stderr, "Error fetching issues: {e}").ok();
            return 1;
        }
    };

    run_tui(result.issues, result.next_page_token, instance, cache).await
}

async fn run_tui(
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
    instance: &Instance,
    cache: &TaskCache<'_>,
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
        jql: MINE_JQL.to_owned(),
        next_page_token,
        detail_links: vec![],
        detail_focused_link: None,
    };
    let exit_code = event_loop(&mut terminal, model, instance, cache).await;

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
        KeyCode::Char('n') => Some(Msg::LoadMore),
        KeyCode::Tab => Some(Msg::FocusNextLink),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        _ => None,
    }
}

/// Outcome of one event-loop turn: either the model to keep drawing with, or the
/// process exit code once the loop is done.
enum StepOutcome {
    Continue(Box<Model>),
    Exit(i32),
}

/// Drives the TUI: on every turn it redraws, then selects over the next terminal
/// event and the reply channel fed by spawned `Cmd` effects (ADR 0008).
async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
) -> i32 {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let mut events = EventStream::new();

    loop {
        let _ = terminal.draw(|frame| view(&model, frame));

        let outcome = tokio::select! {
            event = events.next() => handle_terminal_event(event, model, instance, cache, &tx),
            Some(msg) = rx.recv() => handle_reply(msg, model, instance, cache, &tx),
        };

        model = match outcome {
            StepOutcome::Continue(next_model) => *next_model,
            StepOutcome::Exit(code) => return code,
        };
    }
}

fn handle_terminal_event(
    event: Option<io::Result<Event>>,
    model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> StepOutcome {
    let key = match event {
        Some(Ok(Event::Key(key))) => key,
        Some(Ok(_)) => return StepOutcome::Continue(Box::new(model)),
        Some(Err(_)) | None => return StepOutcome::Exit(1),
    };

    let search_active = model.search.is_some();
    let Some(msg) = map_key_to_msg(key.code, key.modifiers, search_active) else {
        return StepOutcome::Continue(Box::new(model));
    };

    apply_msg(model, msg, instance, cache, tx)
}

/// A reply from a spawned `Cmd` effect. A completed detail fetch is cached here
/// (never inside the spawned task, which owns no borrow of `cache`).
fn handle_reply(
    msg: Msg,
    model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> StepOutcome {
    if let Msg::DetailLoaded(ref issue) = msg {
        cache_detail(cache, &instance.name, issue);
    }
    apply_msg(model, msg, instance, cache, tx)
}

fn apply_msg(
    model: Model,
    msg: Msg,
    instance: &Instance,
    cache: &TaskCache<'_>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> StepOutcome {
    let (next_model, cmds) = update(model, msg);
    if cmds.contains(&Cmd::Quit) {
        return StepOutcome::Exit(0);
    }
    let model = cmds.into_iter().fold(next_model, |m, cmd| {
        dispatch_cmd(cmd, m, instance, cache, tx)
    });
    StepOutcome::Continue(Box::new(model))
}

fn dispatch_cmd(
    cmd: Cmd,
    model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    tx: &mpsc::UnboundedSender<Msg>,
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
        Cmd::LoadDetail(key) => dispatch_load_detail(key, model, instance, cache, tx),
        Cmd::LoadList(jql) => {
            spawn_load_list(jql, instance.clone(), tx.clone());
            model
        }
        Cmd::LoadMore(jql, token) => {
            spawn_load_more(jql, token, instance.clone(), tx.clone());
            model
        }
    }
}

/// Serves a detail fetch from cache synchronously (no fetch needed), otherwise
/// spawns the network fetch and keeps `model.detail == None` (view shows Loading…).
fn dispatch_load_detail(
    key: String,
    model: Model,
    instance: &Instance,
    cache: &TaskCache<'_>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> Model {
    let issue_cache = IssueCache::new(cache.conn());
    match issue_cache.read(&instance.name, &key) {
        Ok(Some(cached)) => update(model, Msg::DetailLoaded(Box::new(cached.issue))).0,
        _ => {
            spawn_load_detail(key, instance.clone(), tx.clone());
            model
        }
    }
}

fn cache_detail(cache: &TaskCache<'_>, instance_name: &str, issue: &Issue) {
    let issue_cache = IssueCache::new(cache.conn());
    let _ = issue_cache.write(instance_name, issue);
}

/// Spawns the detail fetch effect; the result is sent back over `tx` as
/// `Msg::DetailLoaded` on success or `Msg::Back` on error (ADR 0008).
fn spawn_load_detail(key: String, instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match fetch_issue(&instance, &key).await {
            Ok(issue) => Msg::DetailLoaded(Box::new(issue)),
            Err(_) => Msg::Back,
        };
        let _ = tx.send(msg);
    });
}

/// Spawns the list/search fetch effect; the result is sent back over `tx` as
/// `Msg::ListLoaded` on success or `Msg::LoadFailed` on error (ADR 0008).
fn spawn_load_list(jql: String, instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match run_search(&instance, &jql).await {
            Ok(result) => Msg::ListLoaded(result.issues, result.next_page_token),
            Err(e) => Msg::LoadFailed(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

/// Spawns the load-more page fetch effect (ADR 0009); the result is sent back
/// over `tx` as `Msg::MoreLoaded` on success or `Msg::LoadFailed` on error.
/// Opens a fresh client inside the task, mirroring the P1 spawn pattern.
fn spawn_load_more(jql: String, token: String, instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match run_search_page(&instance, &jql, &token).await {
            Ok(result) => Msg::MoreLoaded(result.issues, result.next_page_token),
            Err(e) => Msg::LoadFailed(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

async fn fetch_issue(instance: &Instance, key: &str) -> anyhow::Result<Issue> {
    let client = GouqiJiraClient::new(instance)?;
    client.get_issue(key).await
}

pub(crate) async fn run_search(instance: &Instance, jql: &str) -> anyhow::Result<SearchResult> {
    let client = GouqiJiraClient::new(instance)?;
    client.search(jql, DEFAULT_SEARCH_LIMIT).await
}

async fn run_search_page(
    instance: &Instance,
    jql: &str,
    page_token: &str,
) -> anyhow::Result<SearchResult> {
    let client = GouqiJiraClient::new(instance)?;
    client
        .search_page(jql, DEFAULT_SEARCH_LIMIT, page_token)
        .await
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
