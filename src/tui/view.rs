use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::model::{Model, Screen};
use crate::i18n::t;

const LOADING_NOTICE: &str = "Loading…";
const SEARCH_PROMPT: &str = "JQL> ";
const SEARCH_ERROR_PREFIX: &str = "Error: ";

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

    let footer = Paragraph::new(t("↑/↓ scroll  Esc/b back  q quit")).alignment(Alignment::Center);
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
                .map(|a| a.display_name.clone())
                .unwrap_or_else(|| t("Unassigned"));

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
                "{key}\n{summary}\n\n{status_label}: {status}\n{type_label}: {issue_type}\n{assignee_label}: {assignee}\n\n{description_label}:\n{description}",
                key = issue.key,
                summary = issue.summary,
                status_label = t("Status"),
                status = status_line,
                type_label = t("Type"),
                issue_type = issue.issue_type,
                assignee_label = t("Assignee"),
                assignee = assignee,
                description_label = t("Description"),
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
