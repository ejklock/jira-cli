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
use crate::models::{Issue, IssueRow, Myself, ProjectRow, SearchResult};
use crate::store::cache::{instances_key, IssueCache, TaskCache, TaskListCache};
use crate::store::instances::Instance;

use super::model::{entry_cmds, update, Cmd, Identity, ListOrigin, Model, Msg, Screen};
use super::view::{
    detail_link_at, detail_pos_at, detail_pos_at_clamped, list_click_card, projects_click_row,
    selection_text, view,
};

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

/// The task-list snapshot's max age (ADR 0016 §1): generous, since a
/// revalidation always follows a warm entry.
const TASK_LIST_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;

/// The only list scope entry snapshots (ADR 0016 §4): search results and
/// load-more pages are never snapshotted.
const LIST_SCOPE: &str = "mine";

/// The initial screen the interactive TUI opens on (ADR 0025, BDR 0016). S1
/// constructs `Mine`; S2 adds `Search`; S3 adds `Detail(key)` — a direct
/// seed onto that issue's detail, with no list behind it.
pub enum TuiSeed {
    Mine,
    Search(String),
    Detail(String),
}

/// Entry point for `jira browse`; delegates to [`browse_seeded`] with
/// `TuiSeed::Mine` (BDR 0016 S1/S2 parity — no observable change from before
/// `browse_seeded` existed).
pub async fn browse(
    instance: &Instance,
    cache: &TaskCache<'_>,
    is_tty: bool,
    stderr: &mut impl Write,
) -> i32 {
    browse_seeded(instance, cache, is_tty, TuiSeed::Mine, stderr).await
}

/// Seeded entry point for the interactive TUI (ADR 0025, BDR 0016): routes
/// through the same TTY guard `browse` always has (TtyError preserved), then
/// opens the TUI on `seed`. `TuiSeed::Mine` reuses `fetch_and_run`'s
/// entry-SWR snapshot path unchanged; `TuiSeed::Search` fetches the JQL list
/// and seeds the TUI directly with no snapshot (ADR 0016 §4, mine scope
/// only); `TuiSeed::Detail(key)` resolves that issue cache-or-fetch and seeds
/// the TUI directly on its detail (ADR 0025 §3, BDR 0016 S7).
pub async fn browse_seeded(
    instance: &Instance,
    cache: &TaskCache<'_>,
    is_tty: bool,
    seed: TuiSeed,
    stderr: &mut impl Write,
) -> i32 {
    use crate::cli::{browse_tty_action, BrowseAction};

    match browse_tty_action(is_tty) {
        BrowseAction::TtyError => {
            writeln!(stderr, "{}", t(TTY_ERROR_KEY)).ok();
            1
        }
        BrowseAction::RunTui => match seed {
            TuiSeed::Mine => fetch_and_run(instance, cache, stderr).await,
            TuiSeed::Search(jql) => fetch_and_run_search(&jql, instance, cache, stderr).await,
            TuiSeed::Detail(key) => fetch_and_run_detail(&key, instance, cache, stderr).await,
        },
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
        return run_tui(rows, None, instance, cache, true, &TuiSeed::Mine, None).await;
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
        &TuiSeed::Mine,
        None,
    )
    .await
}

/// `TuiSeed::Search(jql)`'s fetch (ADR 0025 §3, BDR 0016 S5): fetches the
/// JQL list via the same [`run_search`] seam the in-TUI search box uses, then
/// seeds the TUI directly. No snapshot is written — the entry-SWR snapshot
/// is mine-scope only (ADR 0016 §4).
async fn fetch_and_run_search(
    jql: &str,
    instance: &Instance,
    cache: &TaskCache<'_>,
    stderr: &mut impl Write,
) -> i32 {
    let result = match run_search(instance, jql).await {
        Ok(result) => result,
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(stderr, "{}", reauth_message(&instance)).ok();
            return 1;
        }
        Err(e) => {
            writeln!(stderr, "Error running search: {e}").ok();
            return 1;
        }
    };

    run_tui(
        result.issues,
        result.next_page_token,
        instance,
        cache,
        false,
        &TuiSeed::Search(jql.to_owned()),
        None,
    )
    .await
}

/// `TuiSeed::Detail(key)`'s fetch (ADR 0025 §3, BDR 0016 S7): resolves the
/// issue via the same cache-or-fetch seam `dispatch_load_detail` uses inside
/// a running TUI, then seeds the TUI directly on `Screen::Detail`. Rows stay
/// empty — there is no list behind this detail, which is exactly what lets
/// `back_from_detail` (ADR 0025 §3) tell it apart from a drilled-in detail.
async fn fetch_and_run_detail(
    key: &str,
    instance: &Instance,
    cache: &TaskCache<'_>,
    stderr: &mut impl Write,
) -> i32 {
    let issue = match resolve_detail_issue(key, instance, cache).await {
        Ok(issue) => issue,
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(stderr, "{}", reauth_message(&instance)).ok();
            return 1;
        }
        Err(e) => {
            writeln!(stderr, "Error fetching issue: {e}").ok();
            return 1;
        }
    };

    run_tui(
        vec![],
        None,
        instance,
        cache,
        false,
        &TuiSeed::Detail(key.to_owned()),
        Some(issue),
    )
    .await
}

/// Cache-or-fetch seam for a single issue, shared in spirit with
/// `dispatch_load_detail`'s in-TUI path: a cache hit serves synchronously; a
/// miss fetches over the network and warms the cache on success.
async fn resolve_detail_issue(
    key: &str,
    instance: &Instance,
    cache: &TaskCache<'_>,
) -> Result<Issue, ClientError> {
    let issue_cache = IssueCache::new(cache.conn());
    if let Ok(Some(cached)) = issue_cache.read(&instance.name, key) {
        return Ok(cached.issue);
    }
    let issue = fetch_issue(instance, key).await?;
    cache_detail(cache, &instance.name, &issue);
    Ok(issue)
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
    seed: &TuiSeed,
    detail: Option<Issue>,
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

    let model = seeded_model(rows, next_page_token, instance, revalidating, seed);
    let model = seed_detail(model, detail);
    let exit_code = event_loop(&mut terminal, model, instance, cache).await;

    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = disable_raw_mode();
    exit_code
}

/// Builds the initial `Model` for `seed` (ADR 0025, BDR 0016): `TuiSeed::Mine`
/// matches `browse`'s pre-existing entry exactly (`Screen::List`,
/// `ListOrigin::Mine`, the mine JQL); `TuiSeed::Search(jql)` seeds the same
/// `Screen::List` with that JQL and `ListOrigin::Search`; `TuiSeed::Detail`
/// seeds `Screen::Detail` directly, with no JQL/list-origin to restore on
/// Back (empty rows — `back_from_detail` reads that as top-level). The
/// fetched issue itself is applied afterwards by [`seed_detail`].
fn seeded_model(
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
    instance: &Instance,
    revalidating: bool,
    seed: &TuiSeed,
) -> Model {
    let (screen, jql, list_origin) = match seed {
        TuiSeed::Mine => (Screen::List, MINE_JQL.to_owned(), ListOrigin::Mine),
        TuiSeed::Search(jql) => (Screen::List, jql.clone(), ListOrigin::Search),
        TuiSeed::Detail(_) => (Screen::Detail, String::new(), ListOrigin::Mine),
    };
    Model {
        rows,
        selected: 0,
        screen,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: instance.base_url.clone(),
        jql,
        next_page_token,
        detail_links: vec![],
        detail_focused_link: None,
        selection: None,
        identities: vec![Identity {
            email: instance.email.clone(),
            instance: instance.name.clone(),
        }],
        status: None,
        revalidating,
        list_origin,
        projects: vec![],
        projects_selected: 0,
        compose: None,
        detail_focused_comment: None,
        current_account_id: None,
    }
}

/// Applies a fetched `TuiSeed::Detail` issue to `model` through the same
/// `Msg::DetailLoaded` reducer the in-TUI fetch path uses (ADR 0025 §3), so
/// `detail_links`/`detail_focused_link` are derived identically either way.
/// A no-op for `Mine`/`Search` (`detail` is `None`).
fn seed_detail(model: Model, detail: Option<Issue>) -> Model {
    match detail {
        Some(issue) => update(model, Msg::DetailLoaded(Box::new(issue))).0,
        None => model,
    }
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
        'p' => Some(Msg::OpenProjects),
        'c' if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        'c' => Some(Msg::OpenCompose),
        ']' => Some(Msg::FocusNextComment),
        '[' => Some(Msg::FocusPrevComment),
        'e' => Some(Msg::EditFocusedComment),
        _ => None,
    }
}

/// The comment compose's keymap (ADR 0024 §3, BDR 0015 S1-S3): while a
/// compose is open, it owns every key — printable chars append, Enter
/// inserts a newline (never submits), Backspace deletes, Ctrl+S submits, Esc
/// cancels. Any other key (Tab, arrows, …) is a no-op, mirroring
/// `map_key_in_search_mode`'s exclusivity.
pub(super) fn map_key_in_compose_mode(key_code: KeyCode, modifiers: KeyModifiers) -> Option<Msg> {
    match key_code {
        KeyCode::Esc => Some(Msg::CancelCompose),
        KeyCode::Enter => Some(Msg::ComposeNewline),
        KeyCode::Backspace => Some(Msg::ComposeBackspace),
        KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::SubmitCompose),
        KeyCode::Char(c) => Some(Msg::ComposeInput(c)),
        _ => None,
    }
}

/// The pure classification of a mouse event before area-dependent click
/// resolution (ADR 0017 §2-3, ADR 0018 §4, ADR 0019 §3): navigation rides
/// straight through as the existing `Msg`; a left-button down/drag/up
/// candidate carries its terminal (column, row) and the event's modifier
/// set — resolving it into a `Msg` needs a terminal-area `view` read, which
/// this mapper does not have.
pub(super) enum MouseIntent {
    Nav(Msg),
    Click {
        x: u16,
        y: u16,
        /// CONTROL/SUPER gates Detail-screen inline-link activation (ADR
        /// 0018 §4, BDR 0010 S5-S8) and, symmetrically, never starts a
        /// selection (ADR 0019 §3, BDR 0011 S4); the List screen click
        /// resolves the same regardless of modifiers.
        modifiers: KeyModifiers,
    },
    Drag {
        x: u16,
        y: u16,
        modifiers: KeyModifiers,
    },
    Release {
        modifiers: KeyModifiers,
    },
}

/// Maps a raw mouse event to a [`MouseIntent`] (BDR 0009 S1, S2, S7, BDR 0011
/// S1-S4): search mode swallows every mouse event; wheel maps to the
/// existing `Up`/`Down` navigation msgs (screen-awareness comes free from
/// `update_up`/`update_down`); a left-button down/drag/up is a candidate
/// carrying its coordinates and modifier set; the other buttons and moves
/// are not handled (`None`).
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
            modifiers: mouse.modifiers,
        }),
        MouseEventKind::Drag(MouseButton::Left) => Some(MouseIntent::Drag {
            x: mouse.column,
            y: mouse.row,
            modifiers: mouse.modifiers,
        }),
        MouseEventKind::Up(MouseButton::Left) => Some(MouseIntent::Release {
            modifiers: mouse.modifiers,
        }),
        _ => None,
    }
}

/// Resolves a mouse event into a `Msg` (ADR 0017 §2-4, ADR 0018 §4, ADR 0019
/// §3-4): navigation rides straight through; a click/drag/release resolves
/// through [`resolve_click`]/[`resolve_drag`]/[`resolve_release`].
pub(super) fn resolve_mouse_msg(
    mouse: MouseEvent,
    search_active: bool,
    model: &Model,
    area: Rect,
) -> Option<Msg> {
    match map_mouse_to_msg(mouse, search_active)? {
        MouseIntent::Nav(msg) => Some(msg),
        MouseIntent::Click { x, y, modifiers } => resolve_click(model, area, x, y, modifiers),
        MouseIntent::Drag { x, y, modifiers } => resolve_drag(model, area, x, y, modifiers),
        MouseIntent::Release { modifiers } => resolve_release(model, modifiers),
    }
}

/// Resolves a click candidate by screen and modifier set (BDR 0010 S5-S8, BDR
/// 0011 S1/S4, ADR 0021): on the List screen every click (plain or
/// modifier-carrying) resolves through `view::list_click_card` unchanged (B1
/// semantics, S8); on the Projects screen a click resolves through
/// `view::projects_click_row` — the projects analogue (BDR 0013 S2-S3); on
/// the Detail screen a CONTROL/SUPER-carrying click resolves through
/// `view::detail_link_at` (link activation, never a selection); a plain
/// click anchors a selection through `view::detail_pos_at` (ADR 0019 §3).
fn resolve_click(
    model: &Model,
    area: Rect,
    x: u16,
    y: u16,
    modifiers: KeyModifiers,
) -> Option<Msg> {
    if model.screen == Screen::List {
        return list_click_card(model, area, y).map(Msg::CardClicked);
    }
    if model.screen == Screen::Projects {
        return projects_click_row(model, area, y).map(Msg::ProjectClicked);
    }
    let link_modifier = modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
    if link_modifier {
        return detail_link_at(model, area, x, y).map(Msg::LinkClicked);
    }
    detail_pos_at(model, area, x, y).map(Msg::SelStart)
}

/// A left DRAG on the Detail body extends the active selection (ADR 0019
/// §3-4, BDR 0011 S1): a no-op on the List screen (B1 frozen) or with a
/// modifier held (reserved for link activation).
fn resolve_drag(model: &Model, area: Rect, x: u16, y: u16, modifiers: KeyModifiers) -> Option<Msg> {
    if model.screen != Screen::Detail {
        return None;
    }
    if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) {
        return None;
    }
    detail_pos_at_clamped(model, area, x, y).map(Msg::SelDrag)
}

/// A left RELEASE on the Detail body ends the selection gesture (ADR 0019
/// §3, BDR 0011 S2/S3): after a drag it extracts the selected text via
/// `view::selection_text` (the model's `update` copies it and shows the
/// existing "Copied" status); a plain click (no drag) carries `None`,
/// clearing the selection. A no-op on the List screen or with a modifier
/// held (reserved for link activation).
fn resolve_release(model: &Model, modifiers: KeyModifiers) -> Option<Msg> {
    if model.screen != Screen::Detail {
        return None;
    }
    if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) {
        return None;
    }
    let dragged = model.selection.as_ref().is_some_and(|s| s.dragged);
    let text = dragged.then(|| selection_text(model)).flatten();
    Some(Msg::SelEnd(text))
}

/// Outcome of one event-loop turn: either the model to keep drawing with, or the
/// process exit code once the loop is done.
pub(super) enum StepOutcome {
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
    let compose_active = model.compose.is_some();
    let msg = match event {
        Some(Ok(Event::Key(key))) if compose_active => {
            map_key_in_compose_mode(key.code, key.modifiers)
        }
        Some(Ok(Event::Key(key))) => map_key_to_msg(key.code, key.modifiers, search_active),
        // While a compose is open, no mouse event reaches the detail/list
        // machinery (ADR 0024 §3, BDR 0015 S6) — the backdrop is inert.
        Some(Ok(Event::Mouse(_))) if compose_active => None,
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
/// before the guarded swap in `update` runs. The mine-scope snapshot write is
/// bound EXCLUSIVELY to `Msg::RevalidationLoaded` (ADR 0021 §7, BDR 0013 S6)
/// — a project's `Msg::ListLoaded` never reaches it.
pub(super) fn handle_reply(
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
        Cmd::LoadProjects => {
            spawn_load_projects(instance.clone(), tx.clone());
            model
        }
        Cmd::SubmitComment { key, body } => {
            spawn_submit_comment(key, body, instance.clone(), tx.clone());
            model
        }
        Cmd::RefreshDetail(key) => {
            // Bypasses `dispatch_load_detail`'s cache-read gate on purpose
            // (ADR 0024 §5, BDR 0015 S2): a stale cached issue must never
            // shadow the just-posted comment, so this reuses
            // `spawn_load_detail`'s fetch effect and reply mapping directly.
            spawn_load_detail(key, instance.clone(), tx.clone());
            model
        }
        Cmd::LoadMyself => {
            spawn_load_myself(instance.clone(), tx.clone());
            model
        }
        Cmd::EditComment {
            key,
            comment_id,
            body,
        } => {
            spawn_edit_comment(key, comment_id, body, instance.clone(), tx.clone());
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

/// Spawns the projects fetch effect (ADR 0021 §2, BDR 0013 S1/S5); mirrors
/// `spawn_load_list`'s reply-channel and error-string shape (same
/// `Unauthorized -> reauth_message` mapping for E2 parity) but reports
/// through `Msg::ProjectsLoaded`/`Msg::ProjectsFailed`.
fn spawn_load_projects(instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match run_list_projects(&instance).await {
            Ok(rows) => Msg::ProjectsLoaded(rows),
            Err(ClientError::Unauthorized { instance }) => {
                Msg::ProjectsFailed(reauth_message(&instance))
            }
            Err(e) => Msg::ProjectsFailed(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

pub(crate) async fn run_list_projects(instance: &Instance) -> Result<Vec<ProjectRow>, ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.list_projects().await
}

/// Spawns the comment-compose submit effect (ADR 0024 §4, BDR 0015 S2/S4):
/// posts `body` on `key` via the C1 `add_comment` seam and replies
/// `Msg::CommentMutationOk` on success or `Msg::CommentMutationErr` on
/// failure — an Unauthorized (401) carries the same re-auth guidance text
/// every other write/read seam builds (E2 parity).
fn spawn_submit_comment(
    key: String,
    body: String,
    instance: Instance,
    tx: mpsc::UnboundedSender<Msg>,
) {
    tokio::spawn(async move {
        let msg = match submit_comment(&instance, &key, &body).await {
            Ok(()) => Msg::CommentMutationOk,
            Err(ClientError::Unauthorized { instance }) => {
                Msg::CommentMutationErr(reauth_message(&instance))
            }
            Err(e) => Msg::CommentMutationErr(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

pub(crate) async fn submit_comment(
    instance: &Instance,
    key: &str,
    body: &str,
) -> Result<(), ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.add_comment(key, body).await?;
    Ok(())
}

/// Spawns the comment-compose edit-submit effect (ADR 0026 §3, BDR 0017 S4-S5):
/// PUTs `body` onto `comment_id` on `key` via the `update_comment` seam and
/// replies the SAME `Msg::CommentMutationOk`/`Msg::CommentMutationErr`
/// `spawn_submit_comment` uses, mirroring its 401 -> re-auth guidance mapping.
fn spawn_edit_comment(
    key: String,
    comment_id: String,
    body: String,
    instance: Instance,
    tx: mpsc::UnboundedSender<Msg>,
) {
    tokio::spawn(async move {
        let msg = match edit_comment(&instance, &key, &comment_id, &body).await {
            Ok(()) => Msg::CommentMutationOk,
            Err(ClientError::Unauthorized { instance }) => {
                Msg::CommentMutationErr(reauth_message(&instance))
            }
            Err(e) => Msg::CommentMutationErr(e.to_string()),
        };
        let _ = tx.send(msg);
    });
}

pub(crate) async fn edit_comment(
    instance: &Instance,
    key: &str,
    comment_id: &str,
    body: &str,
) -> Result<(), ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.update_comment(key, comment_id, body).await?;
    Ok(())
}

/// Spawns the one-shot authenticated-identity fetch (ADR 0026 §2, BDR 0017
/// S2), dispatched once by `entry_cmds` at browse startup: on success replies
/// `Msg::MyselfLoaded(account_id)`; on failure (offline, auth error) sends
/// nothing at all — `current_account_id` safely stays `None` (safe
/// degradation, never panics or blocks), mirroring `spawn_submit_comment`'s
/// spawn shape.
fn spawn_load_myself(instance: Instance, tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        if let Ok(myself) = fetch_myself(&instance).await {
            let _ = tx.send(Msg::MyselfLoaded(myself.account_id));
        }
    });
}

pub(crate) async fn fetch_myself(instance: &Instance) -> Result<Myself, ClientError> {
    let client = GouqiJiraClient::new(instance).map_err(ClientError::Other)?;
    client.myself().await
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

#[cfg(test)]
#[path = "../../tests/unit/tui/shell.rs"]
mod tests;
