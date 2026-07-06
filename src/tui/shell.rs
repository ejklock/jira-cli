use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use std::io::{self, Write};
use tokio::sync::mpsc;

use crate::client::{ClientError, GouqiJiraClient, JiraClient};
use crate::commands::{reauth_message, DEFAULT_SEARCH_LIMIT, MINE_JQL};
use crate::i18n::t;
use crate::models::{Issue, IssueRow, SearchResult};
use crate::store::cache::{instances_key, IssueCache, TaskCache, TaskListCache};
use crate::store::instances::Instance;

use super::model::{entry_cmds, update, Cmd, Identity, Model, Msg, Screen};
use super::view::{list_click_card, view};

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

/// The task-list snapshot's max age (ADR 0016 §1): generous, since a
/// revalidation always follows a warm entry.
const TASK_LIST_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;

/// The only list scope entry snapshots (ADR 0016 §4): search results and
/// load-more pages are never snapshotted.
const LIST_SCOPE: &str = "mine";

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

/// A warm snapshot (ADR 0016 §1) opens the TUI immediately with no network
/// call; a cold entry keeps the pre-TUI blocking fetch byte-identically
/// (stderr contract incl. E2 401) and seeds the snapshot on success.
pub(crate) async fn fetch_and_run(
    instance: &Instance,
    cache: &TaskCache<'_>,
    stderr: &mut impl Write,
) -> i32 {
    if let Some(rows) = read_snapshot(cache, instance) {
        return run_tui(rows, None, instance, cache, true).await;
    }

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

    write_snapshot(cache, instance, &result.issues);
    run_tui(
        result.issues,
        result.next_page_token,
        instance,
        cache,
        false,
    )
    .await
}

/// Reads the warm task-list snapshot for `instance` (ADR 0016 §1, mine scope
/// only). No row, a stale row (past `TASK_LIST_MAX_AGE_SECS`), or undeserializable
/// JSON are all treated the same: a cold entry, never an error.
pub(super) fn read_snapshot(cache: &TaskCache<'_>, instance: &Instance) -> Option<Vec<IssueRow>> {
    let list_cache = TaskListCache::new(cache.conn());
    let key = instances_key(std::slice::from_ref(instance));
    let list_json = list_cache
        .read(LIST_SCOPE, &key, TASK_LIST_MAX_AGE_SECS)
        .ok()??;
    serde_json::from_str(&list_json).ok()
}

/// Writes the mine-scope task-list snapshot for `instance` (ADR 0016 §4).
/// Serialization/write failures are ignored — the cache is best-effort.
pub(super) fn write_snapshot(cache: &TaskCache<'_>, instance: &Instance, rows: &[IssueRow]) {
    let Ok(list_json) = serde_json::to_string(rows) else {
        return;
    };
    let list_cache = TaskListCache::new(cache.conn());
    let key = instances_key(std::slice::from_ref(instance));
    let _ = list_cache.write(LIST_SCOPE, &key, &list_json);
}

async fn run_tui(
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
    instance: &Instance,
    cache: &TaskCache<'_>,
    revalidating: bool,
) -> i32 {
    let mut stdout = io::stdout();
    if enable_raw_mode().is_err() {
        return 1;
    }
    if execute!(stdout, EnterAlternateScreen, EnableMouseCapture).is_err() {
        let _ = disable_raw_mode();
        return 1;
    }

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => {
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
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
        identities: vec![Identity {
            email: instance.email.clone(),
            instance: instance.name.clone(),
        }],
        status: None,
        revalidating,
    };
    let exit_code = event_loop(&mut terminal, model, instance, cache).await;

    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = disable_raw_mode();
    exit_code
}

fn map_key_to_msg(key_code: KeyCode, modifiers: KeyModifiers, search_active: bool) -> Option<Msg> {
    if search_active {
        return map_key_in_search_mode(key_code, modifiers);
    }
    map_key_in_normal_mode(key_code, modifiers)
}

pub(super) fn map_key_in_search_mode(key_code: KeyCode, modifiers: KeyModifiers) -> Option<Msg> {
    match key_code {
        KeyCode::Enter => Some(Msg::SubmitSearch),
        KeyCode::Esc => Some(Msg::CancelSearch),
        KeyCode::Backspace => Some(Msg::SearchBackspace),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char(c) => Some(Msg::SearchInput(c)),
        _ => None,
    }
}

pub(super) fn map_key_in_normal_mode(key_code: KeyCode, modifiers: KeyModifiers) -> Option<Msg> {
    match key_code {
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::Enter => Some(Msg::Select),
        KeyCode::Esc => Some(Msg::Back),
        KeyCode::Tab => Some(Msg::FocusNextLink),
        KeyCode::Char(c) => map_normal_char_key(c, modifiers),
        _ => None,
    }
}

fn map_normal_char_key(c: char, modifiers: KeyModifiers) -> Option<Msg> {
    match c {
        'k' => Some(Msg::Up),
        'j' => Some(Msg::Down),
        'b' => Some(Msg::Back),
        'q' => Some(Msg::Quit),
        '/' => Some(Msg::OpenSearch),
        'o' => Some(Msg::OpenLink),
        'y' => Some(Msg::CopyKey),
        'n' => Some(Msg::LoadMore),
        'c' if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        _ => None,
    }
}

/// The pure classification of a mouse event before area-dependent click
/// resolution (ADR 0017 §2-3): navigation rides straight through as the
/// existing `Msg`; a left-button-down candidate carries only its terminal
/// (column, row) — resolving it into a `Msg` needs the terminal area
/// `view::list_click_card` reads, which this mapper does not have.
pub(super) enum MouseIntent {
    Nav(Msg),
    Click {
        /// Reserved for R-B3's Detail-screen inline-link click resolution;
        /// this slice resolves clicks by row (`y`) on the list screen only.
        #[allow(dead_code)]
        x: u16,
        y: u16,
    },
}

/// Maps a raw mouse event to a [`MouseIntent`] (BDR 0009 S1, S2, S7): search
/// mode swallows every mouse event; wheel maps to the existing `Up`/`Down`
/// navigation msgs (screen-awareness comes free from `update_up`/`update_down`);
/// a left-button press is a click candidate; drags, the other buttons, moves,
/// and releases are not handled (`None`).
pub(super) fn map_mouse_to_msg(mouse: MouseEvent, search_active: bool) -> Option<MouseIntent> {
    if search_active {
        return None;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(MouseIntent::Nav(Msg::Up)),
        MouseEventKind::ScrollDown => Some(MouseIntent::Nav(Msg::Down)),
        MouseEventKind::Down(MouseButton::Left) => Some(MouseIntent::Click {
            x: mouse.column,
            y: mouse.row,
        }),
        _ => None,
    }
}

/// Resolves a mouse event into a `Msg` (ADR 0017 §2-4): navigation rides
/// straight through; a click resolves through `view::list_click_card` only on
/// the list screen — Detail-screen clicks are a no-op this slice (inline-link
/// activation is R-B3's).
pub(super) fn resolve_mouse_msg(
    mouse: MouseEvent,
    search_active: bool,
    model: &Model,
    area: Rect,
) -> Option<Msg> {
    match map_mouse_to_msg(mouse, search_active)? {
        MouseIntent::Nav(msg) => Some(msg),
        MouseIntent::Click { y, .. } if model.screen == Screen::List => {
            list_click_card(model, area, y).map(Msg::CardClicked)
        }
        MouseIntent::Click { .. } => None,
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

    model = entry_cmds(&model)
        .into_iter()
        .fold(model, |m, cmd| dispatch_cmd(cmd, m, instance, cache, &tx));

    // Tracks the last-drawn frame's area (set by `terminal.draw`'s closure) so
    // a mouse click resolves against exactly what's on screen.
    let mut area = Rect::default();
    loop {
        let _ = terminal.draw(|frame| {
            area = frame.area();
            view(&model, frame);
        });

        let outcome = tokio::select! {
            event = events.next() => handle_terminal_event(event, model, instance, cache, &tx, area),
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
    area: Rect,
) -> StepOutcome {
    let search_active = model.search.is_some();
    let msg = match event {
        Some(Ok(Event::Key(key))) => map_key_to_msg(key.code, key.modifiers, search_active),
        Some(Ok(Event::Mouse(mouse))) => resolve_mouse_msg(mouse, search_active, &model, area),
        Some(Ok(_)) => None,
        Some(Err(_)) | None => return StepOutcome::Exit(1),
    };

    let Some(msg) = msg else {
        return StepOutcome::Continue(Box::new(model));
    };

    apply_msg(model, msg, instance, cache, tx)
}

/// A reply from a spawned `Cmd` effect. A completed detail fetch is cached here
/// (never inside the spawned task, which owns no borrow of `cache`); a
/// completed revalidation rewrites the mine-scope snapshot (BDR 0008 S2)
/// before the guarded swap in `update` runs.
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
    if let Msg::RevalidationLoaded(ref rows, _) = msg {
        write_snapshot(cache, instance, rows);
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
        Cmd::RevalidateList => {
            spawn_revalidate_list(instance.clone(), tx.clone());
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
/// `Msg::DetailLoaded` on success, `Msg::LoadFailed` with the re-auth guidance
/// on an Unauthorized (401), or `Msg::Back` on any other error (ADR 0008).
fn spawn_load_detail(key: String, instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match fetch_issue(&instance, &key).await {
            Ok(issue) => Msg::DetailLoaded(Box::new(issue)),
            Err(ClientError::Unauthorized { instance }) => {
                Msg::LoadFailed(reauth_message(&instance))
            }
            Err(_) => Msg::Back,
        };
        let _ = tx.send(msg);
    });
}

/// Spawns the list/search fetch effect; the result is sent back over `tx` as
/// `Msg::ListLoaded` on success or `Msg::LoadFailed` on error (ADR 0008). An
/// Unauthorized (401) carries the same re-auth guidance text as the CLI.
fn spawn_load_list(jql: String, instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match run_search(&instance, &jql).await {
            Ok(result) => Msg::ListLoaded(result.issues, result.next_page_token),
            Err(ClientError::Unauthorized { instance }) => {
                Msg::LoadFailed(reauth_message(&instance))
            }
            Err(e) => Msg::LoadFailed(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

/// Spawns the entry revalidation fetch (BDR 0008 S1/S2/S5); mirrors
/// `spawn_load_list`'s reply-channel and error-string shape (same
/// `Unauthorized -> reauth_message` mapping for E2 parity) but reports
/// through `Msg::RevalidationLoaded`/`Msg::RevalidationFailed` so the
/// single-flight guard in `update` can tell it apart from a user search.
fn spawn_revalidate_list(instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match run_search(&instance, MINE_JQL).await {
            Ok(result) => Msg::RevalidationLoaded(result.issues, result.next_page_token),
            Err(ClientError::Unauthorized { instance }) => {
                Msg::RevalidationFailed(reauth_message(&instance))
            }
            Err(e) => Msg::RevalidationFailed(e.to_string()),
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
            Err(ClientError::Unauthorized { instance }) => {
                Msg::LoadFailed(reauth_message(&instance))
            }
            Err(e) => Msg::LoadFailed(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

async fn fetch_issue(instance: &Instance, key: &str) -> Result<Issue, ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.get_issue(key).await
}

pub(crate) async fn run_search(
    instance: &Instance,
    jql: &str,
) -> Result<SearchResult, ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.search(jql, DEFAULT_SEARCH_LIMIT).await
}

async fn run_search_page(
    instance: &Instance,
    jql: &str,
    page_token: &str,
) -> Result<SearchResult, ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
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
