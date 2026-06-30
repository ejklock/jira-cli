use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use std::io::{self, Write};

use crate::client::{GouqiJiraClient, JiraClient};
use crate::commands::{DEFAULT_SEARCH_LIMIT, MINE_JQL};
use crate::i18n::t;
use crate::models::{Issue, IssueRow};
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";
const LOADING_NOTICE: &str = "Loading…";
const SEARCH_PROMPT: &str = "JQL> ";
const SEARCH_ERROR_PREFIX: &str = "Error: ";

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Detail,
}

pub struct Model {
    pub rows: Vec<IssueRow>,
    pub selected: usize,
    pub screen: Screen,
    pub detail: Option<Issue>,
    pub detail_scroll: u16,
    pub search: Option<String>,
    pub error: Option<String>,
}

pub enum Msg {
    Up,
    Down,
    Quit,
    Select,
    Back,
    DetailLoaded(Box<Issue>),
    OpenSearch,
    SearchInput(char),
    SearchBackspace,
    SubmitSearch,
    CancelSearch,
    ListLoaded(Vec<IssueRow>),
    LoadFailed(String),
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Quit,
    LoadDetail(String),
    LoadList(String),
}

/// Pure state transition — no I/O, no terminal, no clock.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::Up => update_up(model),
        Msg::Down => update_down(model),
        Msg::Quit => (model, vec![Cmd::Quit]),
        Msg::Select => update_select(model),
        Msg::Back => update_back(model),
        Msg::DetailLoaded(issue) => update_detail_loaded(model, issue),
        Msg::OpenSearch => update_open_search(model),
        Msg::SearchInput(c) => update_search_input(model, c),
        Msg::SearchBackspace => update_search_backspace(model),
        Msg::SubmitSearch => update_submit_search(model),
        Msg::CancelSearch => update_cancel_search(model),
        Msg::ListLoaded(rows) => update_list_loaded(model, rows),
        Msg::LoadFailed(msg) => update_load_failed(model, msg),
    }
}

fn update_down(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => {
            let last = model.rows.len().saturating_sub(1);
            let selected = (model.selected + 1).min(last);
            (Model { selected, ..model }, vec![])
        }
        Screen::Detail => {
            let detail_scroll = model.detail_scroll.saturating_add(1);
            (
                Model {
                    detail_scroll,
                    ..model
                },
                vec![],
            )
        }
    }
}

fn update_up(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => {
            let selected = model.selected.saturating_sub(1);
            (Model { selected, ..model }, vec![])
        }
        Screen::Detail => {
            let detail_scroll = model.detail_scroll.saturating_sub(1);
            (
                Model {
                    detail_scroll,
                    ..model
                },
                vec![],
            )
        }
    }
}

fn update_select(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List || model.rows.is_empty() {
        return (model, vec![]);
    }
    let key = model.rows[model.selected].key.clone();
    let next = Model {
        screen: Screen::Detail,
        detail: None,
        detail_scroll: 0,
        ..model
    };
    (next, vec![Cmd::LoadDetail(key)])
}

fn update_back(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        screen: Screen::List,
        detail: None,
        ..model
    };
    (next, vec![])
}

fn update_detail_loaded(model: Model, issue: Box<Issue>) -> (Model, Vec<Cmd>) {
    let next = Model {
        detail: Some(*issue),
        detail_scroll: 0,
        ..model
    };
    (next, vec![])
}

fn update_open_search(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List {
        return (model, vec![]);
    }
    let next = Model {
        search: Some(String::new()),
        error: None,
        ..model
    };
    (next, vec![])
}

fn update_search_input(model: Model, c: char) -> (Model, Vec<Cmd>) {
    let search = match model.search {
        Some(mut q) => {
            q.push(c);
            Some(q)
        }
        None => None,
    };
    (Model { search, ..model }, vec![])
}

fn update_search_backspace(model: Model) -> (Model, Vec<Cmd>) {
    let search = match model.search {
        Some(mut q) => {
            q.pop();
            Some(q)
        }
        None => None,
    };
    (Model { search, ..model }, vec![])
}

fn update_submit_search(model: Model) -> (Model, Vec<Cmd>) {
    match &model.search {
        Some(q) if !q.is_empty() => {
            let jql = q.clone();
            (model, vec![Cmd::LoadList(jql)])
        }
        _ => (model, vec![]),
    }
}

fn update_cancel_search(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        search: None,
        ..model
    };
    (next, vec![])
}

fn update_list_loaded(model: Model, rows: Vec<IssueRow>) -> (Model, Vec<Cmd>) {
    let next = Model {
        rows,
        selected: 0,
        search: None,
        error: None,
        ..model
    };
    (next, vec![])
}

fn update_load_failed(model: Model, msg: String) -> (Model, Vec<Cmd>) {
    let next = Model {
        error: Some(msg),
        search: None,
        ..model
    };
    (next, vec![])
}

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

/// Pure rendering function — maps Model to ratatui widgets.
/// Works with any backend including TestBackend.
pub fn view(model: &Model, frame: &mut Frame) {
    match model.screen {
        Screen::List => view_list(model, frame),
        Screen::Detail => view_detail(model, frame),
    }
}

fn view_list(model: &Model, frame: &mut Frame) {
    let area = frame.area();

    let has_search_bar = model.search.is_some();
    let has_error_banner = model.error.is_some();

    let mut constraints = vec![Constraint::Length(1)];
    if has_search_bar {
        constraints.push(Constraint::Length(1));
    }
    if has_error_banner {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 1usize;

    if has_search_bar {
        let query = model.search.as_deref().unwrap_or("");
        let input_line = Paragraph::new(format!("{SEARCH_PROMPT}{query}"));
        frame.render_widget(input_line, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    if has_error_banner {
        let msg = model.error.as_deref().unwrap_or("");
        let banner = Paragraph::new(format!("{SEARCH_ERROR_PREFIX}{msg}"))
            .style(Style::default().add_modifier(Modifier::BOLD));
        frame.render_widget(banner, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    let table_chunk = chunks[chunk_idx];
    let footer_chunk = chunks[chunk_idx + 1];

    let header_cells = [
        t("KEY"),
        t("TYPE"),
        t("STATUS"),
        t("ASSIGNEE"),
        t("SUMMARY"),
    ]
    .into_iter()
    .map(|h| Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD)));

    let header_row = Row::new(header_cells).height(1);

    let widths = [
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(20),
        Constraint::Min(20),
    ];

    if model.rows.is_empty() {
        let table = Table::new([Row::new([Cell::from(t("No issues."))])], widths)
            .header(header_row)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        frame.render_widget(table, table_chunk);
    } else {
        let data_rows: Vec<Row> = model
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let assignee = row
                    .assignee
                    .as_deref()
                    .map(str::to_owned)
                    .unwrap_or_else(|| t("Unassigned"));

                let cells = [
                    Cell::from(row.key.clone()),
                    Cell::from(row.issue_type.clone()),
                    Cell::from(row.status.clone()),
                    Cell::from(assignee),
                    Cell::from(row.summary.clone()),
                ];

                if i == model.selected {
                    Row::new(cells).style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    Row::new(cells)
                }
            })
            .collect();

        let table = Table::new(data_rows, widths).header(header_row).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        frame.render_widget(table, table_chunk);
    }

    let hint_text = if has_search_bar {
        "Enter submit  Esc cancel  Backspace delete"
    } else {
        "↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"
    };
    let hint = Paragraph::new(hint_text).alignment(Alignment::Center);
    frame.render_widget(hint, footer_chunk);
}

/// Pure detail view — renders the loaded issue or a loading notice.
pub fn view_detail(model: &Model, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let footer =
        Paragraph::new(t("↑/↓ scroll  r refresh  Esc/b back  q quit")).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);

    match &model.detail {
        None => {
            let notice = Paragraph::new(t(LOADING_NOTICE)).alignment(Alignment::Center);
            frame.render_widget(notice, chunks[1]);
        }
        Some(issue) => {
            let assignee = issue
                .assignee
                .as_ref()
                .map(|a| a.display_name.as_str())
                .unwrap_or("Unassigned");

            let status_line = match &issue.status_category {
                Some(cat) => format!("{} ({})", issue.status, cat),
                None => issue.status.clone(),
            };

            let description = issue
                .description
                .as_deref()
                .map(crate::render::adf_to_plain_text)
                .unwrap_or_default();

            let body = format!(
                "{key}\n{summary}\n\nStatus: {status}\nType: {issue_type}\nAssignee: {assignee}\n\nDescription:\n{description}",
                key = issue.key,
                summary = issue.summary,
                status = status_line,
                issue_type = issue.issue_type,
                assignee = assignee,
                description = description,
            );

            let paragraph = Paragraph::new(body)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .wrap(Wrap { trim: false })
                .scroll((model.detail_scroll, 0));

            frame.render_widget(paragraph, chunks[1]);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/tui.rs"]
mod tests;
