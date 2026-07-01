use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::model::{Model, Screen};
use crate::i18n::t;
use crate::models::{Issue, IssueRow};
use crate::render::{adf_to_rich, relative_due, today_days_now, RichLine, RichSpan, RichStyle};

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

    let chunks = list_layout_chunks(area, has_search_bar, has_error_banner);
    let mut chunk_idx = 1usize;

    if has_search_bar {
        render_search_bar(
            frame,
            chunks[chunk_idx],
            model.search.as_deref().unwrap_or(""),
        );
        chunk_idx += 1;
    }

    if has_error_banner {
        render_error_banner(
            frame,
            chunks[chunk_idx],
            model.error.as_deref().unwrap_or(""),
        );
        chunk_idx += 1;
    }

    render_list_table(frame, chunks[chunk_idx], model);

    let hint = Paragraph::new(list_footer_hint(model, has_search_bar)).alignment(Alignment::Center);
    frame.render_widget(hint, chunks[chunk_idx + 1]);
}

/// Builds the `view_list` vertical layout: the optional search bar and error
/// banner rows are only reserved when active, sandwiched between the fixed
/// top row and the table/footer pair.
fn list_layout_chunks(
    area: Rect,
    has_search_bar: bool,
    has_error_banner: bool,
) -> std::rc::Rc<[Rect]> {
    let mut constraints = vec![Constraint::Length(1)];
    if has_search_bar {
        constraints.push(Constraint::Length(1));
    }
    if has_error_banner {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
}

fn render_search_bar(frame: &mut Frame, chunk: Rect, query: &str) {
    let input_line = Paragraph::new(format!("{}{query}", t(SEARCH_PROMPT)));
    frame.render_widget(input_line, chunk);
}

fn render_error_banner(frame: &mut Frame, chunk: Rect, msg: &str) {
    let banner = Paragraph::new(format!("{}{msg}", t(SEARCH_ERROR_PREFIX)))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(banner, chunk);
}

fn render_list_table(frame: &mut Frame, chunk: Rect, model: &Model) {
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

    let table = if model.rows.is_empty() {
        Table::new([Row::new([Cell::from(t("No issues."))])], widths)
    } else {
        let data_rows: Vec<Row> = model
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| list_row(row, i == model.selected))
            .collect();
        Table::new(data_rows, widths)
    }
    .header(header_row)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    frame.render_widget(table, chunk);
}

fn list_row(row: &IssueRow, is_selected: bool) -> Row<'static> {
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

    if is_selected {
        Row::new(cells).style(Style::default().add_modifier(Modifier::REVERSED))
    } else {
        Row::new(cells)
    }
}

fn list_footer_hint(model: &Model, has_search_bar: bool) -> String {
    let base_hint = if has_search_bar {
        t("Enter submit  Esc cancel  Backspace delete")
    } else {
        t("↑/↓ navigate  /  search  Enter select  Esc/b back  q quit")
    };

    if !has_search_bar && model.next_page_token.is_some() {
        format!("{base_hint}  {}", t("n more"))
    } else {
        base_hint
    }
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
        Paragraph::new(t("↑/↓ j/k scroll  Esc/b back  q quit")).alignment(Alignment::Center);
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
                .map(adf_to_rich)
                .unwrap_or_default();

            let mut lines = vec![
                Line::from(issue.key.clone()),
                Line::from(issue.summary.clone()),
                Line::from(""),
                Line::from(format!("{}: {status_line}", t("Status"))),
                Line::from(format!("{}: {}", t("Type"), issue.issue_type)),
                Line::from(format!("{}: {assignee}", t("Assignee"))),
            ];
            lines.extend(due_line(issue));
            lines.push(Line::from(""));
            lines.push(Line::from(format!("{}:", t("Description"))));
            lines.extend(description_lines_to_ratatui(
                &description,
                model.detail_focused_link,
            ));
            lines.extend(detail_comment_lines(issue));

            let paragraph = Paragraph::new(Text::from(lines))
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

/// Builds the optional `Due: {relative}` line inserted after the Assignee line
/// in `view_detail`, reusing A3a's `relative_due` formatter (no duplicated date
/// math). Returns `None` when the issue has no `duedate` or it fails to parse,
/// so `view_detail` renders no `Due` line at all (mirrors the CLI `get` line).
fn due_line(issue: &Issue) -> Option<Line<'static>> {
    let duedate = issue.duedate.as_deref()?;
    let relative = relative_due(duedate, today_days_now())?;
    Some(Line::from(format!("{}: {relative}", t("Due"))))
}

/// Builds the comment section appended after the description in `view_detail`,
/// mirroring `render_comment_human`'s layout (header + `adf_to_rich`-styled body)
/// but for ratatui. Returns an empty `Vec` when the issue has no comments, so
/// `view_detail` renders no `Comments:` section at all (mirrors the CLI).
fn detail_comment_lines(issue: &Issue) -> Vec<Line<'static>> {
    if issue.comments.is_empty() {
        return vec![];
    }

    let mut lines = vec![Line::from(""), Line::from(format!("{}:", t("Comments")))];
    for comment in &issue.comments {
        lines.extend(comment_lines(comment));
    }
    lines
}

/// Renders a single comment's header + styled body + trailing blank line.
fn comment_lines(comment: &crate::models::IssueComment) -> Vec<Line<'static>> {
    let author = comment
        .author
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| t("Unknown"));
    let created = comment.created.as_deref().unwrap_or("");

    let mut lines = vec![Line::from(format!("[{author}] {created}"))];

    let mut link_occurrence = 0usize;
    let body = adf_to_rich(&comment.body);
    lines.extend(
        body.iter()
            .map(|line| rich_line_to_ratatui(line, None, &mut link_occurrence)),
    );
    lines.push(Line::from(""));
    lines
}

fn rich_style_to_ratatui(style: &RichStyle) -> Style {
    let mut result = Style::default();
    if style.bold {
        result = result.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        result = result.add_modifier(Modifier::ITALIC);
    }
    if style.strike {
        result = result.add_modifier(Modifier::CROSSED_OUT);
    }
    if style.underline || style.link.is_some() {
        result = result.add_modifier(Modifier::UNDERLINED);
    }
    if style.code {
        result = result.add_modifier(Modifier::DIM);
    }
    result
}

/// Maps the description's rich lines to ratatui lines, reversing the style of
/// the inline link whose render-order occurrence matches `focused_link`.
fn description_lines_to_ratatui(
    description: &[RichLine],
    focused_link: Option<usize>,
) -> Vec<Line<'static>> {
    let mut link_occurrence = 0usize;
    description
        .iter()
        .map(|line| rich_line_to_ratatui(line, focused_link, &mut link_occurrence))
        .collect()
}

fn rich_line_to_ratatui(
    line: &RichLine,
    focused_link: Option<usize>,
    link_occurrence: &mut usize,
) -> Line<'static> {
    Line::from(
        line.iter()
            .map(|span| {
                let style = span_style(span, focused_link, link_occurrence);
                ratatui::text::Span::styled(span.text.clone(), style)
            })
            .collect::<Vec<_>>(),
    )
}

fn span_style(span: &RichSpan, focused_link: Option<usize>, link_occurrence: &mut usize) -> Style {
    let style = rich_style_to_ratatui(&span.style);
    if span.style.link.is_none() {
        return style;
    }
    let is_focused = focused_link == Some(*link_occurrence);
    *link_occurrence += 1;
    if is_focused {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    }
}
