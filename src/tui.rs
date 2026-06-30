use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::io::{self, Write};

use crate::client::{GouqiJiraClient, JiraClient};
use crate::commands::{DEFAULT_SEARCH_LIMIT, MINE_JQL};
use crate::i18n::t;
use crate::models::IssueRow;
use crate::store::instances::Instance;

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

pub struct Model {
    pub rows: Vec<IssueRow>,
    pub selected: usize,
}

pub enum Msg {
    Up,
    Down,
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Quit,
}

/// Pure state transition — no I/O, no terminal, no clock.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::Down => {
            let last = model.rows.len().saturating_sub(1);
            let selected = (model.selected + 1).min(last);
            (Model { selected, ..model }, vec![])
        }
        Msg::Up => {
            let selected = model.selected.saturating_sub(1);
            (Model { selected, ..model }, vec![])
        }
        Msg::Quit => (model, vec![Cmd::Quit]),
    }
}

/// Entry point for `jira browse`.
///
/// Checks the TTY guard, then fetches the mine list and enters the raw-mode draw loop.
/// Returns 0 on clean quit (`q` or Ctrl+C) and 1 on the non-TTY guard path or fetch error.
pub async fn browse(instance: &Instance, is_tty: bool, stderr: &mut impl Write) -> i32 {
    use crate::cli::{browse_tty_action, BrowseAction};

    match browse_tty_action(is_tty) {
        BrowseAction::TtyError => {
            writeln!(stderr, "{}", t(TTY_ERROR_KEY)).ok();
            1
        }
        BrowseAction::RunTui => fetch_and_run(instance, stderr).await,
    }
}

async fn fetch_and_run(instance: &Instance, stderr: &mut impl Write) -> i32 {
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

    run_tui(rows)
}

fn run_tui(rows: Vec<IssueRow>) -> i32 {
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

    let model = Model { rows, selected: 0 };
    let exit_code = draw_loop(&mut terminal, model);

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    exit_code
}

fn draw_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut model: Model) -> i32 {
    loop {
        let _ = terminal.draw(|frame| view(&model, frame));

        match event::read() {
            Ok(Event::Key(key)) => {
                let msg = if key.code == KeyCode::Up {
                    Some(Msg::Up)
                } else if key.code == KeyCode::Down {
                    Some(Msg::Down)
                } else if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    Some(Msg::Quit)
                } else {
                    None
                };

                if let Some(m) = msg {
                    let (next_model, cmds) = update(model, m);
                    model = next_model;
                    if cmds.contains(&Cmd::Quit) {
                        return 0;
                    }
                }
            }
            Err(_) => return 1,
            Ok(_) => {}
        }
    }
}

/// Pure rendering function — maps Model to ratatui widgets.
/// Works with any backend including TestBackend.
pub fn view(model: &Model, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

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
        frame.render_widget(table, chunks[1]);
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
        frame.render_widget(table, chunks[1]);
    }

    let hint = Paragraph::new(t(
        "↑/↓ navigate  Enter select  r refresh  Esc/b back  q quit",
    ))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[2]);
}

#[cfg(test)]
#[path = "../tests/unit/tui.rs"]
mod tests;
