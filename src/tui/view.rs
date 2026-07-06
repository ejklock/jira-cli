use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::model::{header_line, Model, Screen};
use super::panel;
use super::theme;
use crate::i18n::t;
use crate::models::{Issue, IssueComment, IssueRow};
use crate::render::{
    adf_to_rich, due_day_delta, relative_due, today_days_now, RichLine, RichSpan, RichStyle,
};

/// The fixed terminal-row height of one list card (BDR 0007 S2): rounded top
/// border, `KEY summary`, `{due} · {status} · {project}`, rounded bottom
/// border.
const CARD_HEIGHT: u16 = 4;

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

/// Renders the identity header bar (ADR 0014 §2, BDR 0007 S1) into the
/// screen's reserved top row, themed via `theme::header_bar()`.
fn render_header(frame: &mut Frame, chunk: Rect, model: &Model) {
    let header = Paragraph::new(header_line(&model.identities)).style(theme::header_bar());
    frame.render_widget(header, chunk);
}

fn view_list(model: &Model, frame: &mut Frame) {
    let area = frame.area();

    let has_search_bar = model.search.is_some();
    let has_error_banner = model.error.is_some();

    let chunks = list_layout_chunks(area, has_search_bar, has_error_banner);
    render_header(frame, chunks[0], model);
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

    render_list_cards(frame, chunks[chunk_idx], model);

    let hint = Paragraph::new(list_footer_hint(model, has_search_bar))
        .alignment(Alignment::Center)
        .style(theme::footer());
    frame.render_widget(hint, chunks[chunk_idx + 1]);
}

/// Builds the `view_list` vertical layout: the header occupies the fixed top
/// row; the optional search bar and error banner rows are only reserved when
/// active, sandwiched between the header and the table/footer pair.
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

/// Renders the list content region as stacked per-issue cards (BDR 0007 S2-S4),
/// windowing the visible slice so the selected card always stays in view.
fn render_list_cards(frame: &mut Frame, chunk: Rect, model: &Model) {
    if model.rows.is_empty() {
        let notice = Paragraph::new(t("No issues.")).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        frame.render_widget(notice, chunk);
        return;
    }

    let today = today_days_now();
    let visible = (chunk.height / CARD_HEIGHT).max(1) as usize;
    let first = first_visible_card(model.selected, model.rows.len(), visible);
    let last = (first + visible).min(model.rows.len());

    for (offset, row) in model.rows[first..last].iter().enumerate() {
        let index = first + offset;
        let card_area = Rect {
            x: chunk.x,
            y: chunk.y + (offset as u16 * CARD_HEIGHT),
            width: chunk.width,
            height: CARD_HEIGHT,
        };
        if card_area.y + CARD_HEIGHT > chunk.y + chunk.height {
            break;
        }
        render_card(frame, card_area, row, index == model.selected, today);
    }
}

/// Computes the first visible card index for the list's scroll window (BDR
/// 0007 S4): keeps `selected` inside `[first, first + visible)`, clamped so
/// the window never runs past the last card or before index 0.
pub(crate) fn first_visible_card(selected: usize, count: usize, visible: usize) -> usize {
    if visible == 0 || count <= visible {
        return 0;
    }
    let max_first = count - visible;
    let first = selected.saturating_sub(visible.saturating_sub(1));
    first.min(max_first)
}

/// Renders one issue card: a rounded-border block, `KEY summary` on the first
/// content line, `{due} · {status} · {project}` on the second. A selected
/// card is styled uniformly (border + both lines) with `theme::selected()`.
fn render_card(frame: &mut Frame, area: Rect, row: &IssueRow, is_selected: bool, today: i64) {
    let card_style = is_selected.then(theme::selected).unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(card_style);

    let override_style = is_selected.then(theme::selected);
    let lines = vec![
        Line::from(card_title_spans(row, override_style)),
        Line::from(card_meta_spans(row, today, override_style)),
    ];

    let paragraph = Paragraph::new(Text::from(lines)).block(block);
    frame.render_widget(paragraph, area);
}

/// Builds the card's first line: `KEY summary`, the key styled as a badge
/// (or overridden to the selection style when the card is selected).
fn card_title_spans(row: &IssueRow, override_style: Option<Style>) -> Vec<Span<'static>> {
    let key_style = override_style.unwrap_or_else(theme::badge);
    let rest_style = override_style.unwrap_or_default();
    vec![
        Span::styled(row.key.clone(), key_style),
        Span::styled(format!(" {}", row.summary), rest_style),
    ]
}

/// One labeled value in a list card's meta line (BDR 0007 S2/S3): its display
/// text and the style bucket it renders in.
struct CardSegment {
    text: String,
    style: Style,
}

/// Builds the card's second line: `{due} · {status} · {project}`. The due
/// segment always renders (falling back to the i18n "no due date" text);
/// status and project are omitted when empty/absent, leaving no dangling
/// `·` separator.
fn card_meta_spans(
    row: &IssueRow,
    today_days: i64,
    override_style: Option<Style>,
) -> Vec<Span<'static>> {
    let segments = card_meta_segments(row, today_days);
    let mut spans = Vec::with_capacity(segments.len() * 2);
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " · ".to_owned(),
                override_style.unwrap_or_default(),
            ));
        }
        let style = override_style.unwrap_or(segment.style);
        spans.push(Span::styled(segment.text.clone(), style));
    }
    spans
}

/// The card's second-line text with no styling — `{due} · {status} · {project}`
/// with empty/absent segments omitted (no dangling `·`). Exposed for pure
/// unit tests of segment omission; [`card_meta_spans`] is the styled variant
/// the renderer actually uses.
#[cfg(test)]
pub(crate) fn card_meta_line(row: &IssueRow, today_days: i64) -> String {
    card_meta_segments(row, today_days)
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn card_meta_segments(row: &IssueRow, today_days: i64) -> Vec<CardSegment> {
    let mut segments = vec![due_card_segment(row, today_days)];
    if !row.status.is_empty() {
        segments.push(CardSegment {
            text: row.status.clone(),
            style: Style::default(),
        });
    }
    if let Some(project) = row.project.as_deref().filter(|p| !p.is_empty()) {
        segments.push(CardSegment {
            text: project.to_owned(),
            style: Style::default(),
        });
    }
    segments
}

/// Builds the due segment: the localized relative-due text colored by
/// [`due_delta_style`], or the i18n "no due date" text in the default style
/// when the row has no (parseable) due date.
fn due_card_segment(row: &IssueRow, today_days: i64) -> CardSegment {
    let parsed = row
        .duedate
        .as_deref()
        .and_then(|d| due_day_delta(d, today_days).map(|delta| (d, delta)));

    match parsed {
        Some((duedate, delta)) => CardSegment {
            text: relative_due(duedate, today_days).unwrap_or_default(),
            style: due_delta_style(delta),
        },
        None => CardSegment {
            text: t("no due date"),
            style: Style::default(),
        },
    }
}

/// Maps a due day-delta to its display style (ADR 0014 §1): overdue red,
/// near (0-2 days out) amber, otherwise the default style.
fn due_delta_style(delta: i64) -> Style {
    if delta < 0 {
        theme::due_overdue()
    } else if (0..=2).contains(&delta) {
        theme::due_near()
    } else {
        Style::default()
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

/// The frame border + the scroll-content region's top/bottom rows (BDR 0007
/// S6 clamp math needs the same figure the renderer subtracts).
const DETAIL_FRAME_BORDER_ROWS: u16 = 2;
const DETAIL_FRAME_BORDER_COLS: u16 = 2;

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

    render_header(frame, chunks[0], model);

    let footer = Paragraph::new(t("↑/↓ j/k scroll  Esc/b back  q quit"))
        .alignment(Alignment::Center)
        .style(theme::footer());
    frame.render_widget(footer, chunks[2]);

    match &model.detail {
        None => {
            let notice = Paragraph::new(t(LOADING_NOTICE)).alignment(Alignment::Center);
            frame.render_widget(notice, chunks[1]);
        }
        Some(issue) => render_detail_panels(
            frame,
            chunks[1],
            issue,
            model.detail_focused_link,
            model.detail_scroll,
        ),
    }
}

/// Renders the detail frame (ADR 0014 §4, BDR 0007 S5-S6): the issue summary
/// as the border title, the Details/Description/Comments panels as one
/// globally-scrolled `Paragraph`, and a `Scrollbar` when content overflows
/// the viewport. The offset clamps to the content's last page so scrolling
/// past the end never leaves blank overscroll.
fn render_detail_panels(
    frame: &mut Frame,
    area: Rect,
    issue: &Issue,
    focused_link: Option<usize>,
    scroll: u16,
) {
    let inner_width = area.width.saturating_sub(DETAIL_FRAME_BORDER_COLS);
    let lines = build_detail_lines(issue, focused_link, inner_width);
    let viewport_height = area.height.saturating_sub(DETAIL_FRAME_BORDER_ROWS);
    let total_lines = lines.len() as u16;
    let effective_offset = clamp_scroll_offset(scroll, total_lines, viewport_height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(detail_frame_title(issue, area.width));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((effective_offset, 0));
    frame.render_widget(paragraph, area);

    if total_lines > viewport_height {
        render_detail_scrollbar(frame, area, total_lines, effective_offset);
    }
}

/// Clamps a scroll offset to the last page of `total_lines` given
/// `viewport_height` (BDR 0007 S6): scrolling past the end lands the last
/// content line at the bottom row instead of leaving blank overscroll.
pub(crate) fn clamp_scroll_offset(offset: u16, total_lines: u16, viewport_height: u16) -> u16 {
    offset.min(total_lines.saturating_sub(viewport_height))
}

fn render_detail_scrollbar(frame: &mut Frame, area: Rect, total_lines: u16, offset: u16) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None);
    let mut state = ScrollbarState::new(total_lines as usize).position(offset as usize);
    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// The detail frame's border title (ADR 0014 §4): the issue summary,
/// ellipsized to the frame width, falling back to the key when the summary
/// is empty.
fn detail_frame_title(issue: &Issue, frame_width: u16) -> String {
    let raw = if issue.summary.trim().is_empty() {
        issue.key.as_str()
    } else {
        issue.summary.as_str()
    };
    let budget = frame_width.saturating_sub(DETAIL_FRAME_BORDER_COLS + 2);
    panel::ellipsize_display(raw, budget)
}

/// Composes the single globally-scrolled line buffer (BDR 0007 S5): the
/// Details meta panel, the Description panel, and — when present — the
/// Comments panel, each drawn via `panel::panel_box` at the same `width`.
fn build_detail_lines(
    issue: &Issue,
    focused_link: Option<usize>,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = details_panel(issue, width);
    lines.push(Line::from(""));
    lines.extend(description_panel(issue, focused_link, width));
    if let Some(comments) = comments_panel(issue, width) {
        lines.push(Line::from(""));
        lines.extend(comments);
    }
    lines
}

/// The Details panel (BDR 0007 S5): a 2-column meta table — Title, Key,
/// Status, Type, Assignee, and Due (omitted when absent/unparseable).
fn details_panel(issue: &Issue, width: u16) -> Vec<Line<'static>> {
    panel::panel_box(&t("Details"), details_meta_rows(issue, width), width)
}

fn details_meta_rows(issue: &Issue, width: u16) -> Vec<Line<'static>> {
    let inner_width = panel::inner_content_width(width);
    let mut rows = vec![
        meta_row(&t("Title"), &issue.summary, inner_width),
        meta_row(&t("Key"), &issue.key, inner_width),
        meta_row(&t("Status"), &status_text(issue), inner_width),
        meta_row(&t("Type"), &issue.issue_type, inner_width),
        meta_row(&t("Assignee"), &assignee_text(issue), inner_width),
    ];
    if let Some(due) = due_relative_text(issue) {
        rows.push(meta_row(&t("Due"), &due, inner_width));
    }
    rows
}

fn meta_row(label: &str, value: &str, inner_width: u16) -> Line<'static> {
    let prefix = format!("{label}: ");
    let budget = inner_width.saturating_sub(UnicodeWidthStr::width(prefix.as_str()) as u16);
    let value = panel::ellipsize_display(value, budget);
    Line::from(format!("{prefix}{value}"))
}

fn status_text(issue: &Issue) -> String {
    match &issue.status_category {
        Some(cat) => format!("{} ({})", issue.status, cat),
        None => issue.status.clone(),
    }
}

fn assignee_text(issue: &Issue) -> String {
    issue
        .assignee
        .as_ref()
        .map(|a| a.display_name.clone())
        .unwrap_or_else(|| t("Unassigned"))
}

/// The issue's due date as A3a's `relative_due` text, reused by both the
/// Details panel's Due row and (in earlier slices) the flat `Due:` line.
/// `None` when the issue has no `duedate` or it fails to parse, so the
/// Details panel renders no Due row at all.
fn due_relative_text(issue: &Issue) -> Option<String> {
    let duedate = issue.duedate.as_deref()?;
    relative_due(duedate, today_days_now())
}

/// The Description panel (BDR 0007 S5): the existing styled ADF lines,
/// wrapped to the panel's inner width before boxing so no line exceeds it
/// (offset math stays exact) and focused-link `REVERSED` styling survives.
fn description_panel(issue: &Issue, focused_link: Option<usize>, width: u16) -> Vec<Line<'static>> {
    let description = issue
        .description
        .as_deref()
        .map(adf_to_rich)
        .unwrap_or_default();
    let styled = description_lines_to_ratatui(&description, focused_link);
    let inner_width = panel::inner_content_width(width);
    let wrapped = wrap_lines_to_width(styled, inner_width);
    panel::panel_box(&t("Description"), wrapped, width)
}

/// The Comments panel (BDR 0007 S5): titled `Comments (N)`, its body made of
/// nested per-comment cards (empty label, author `[created]` header line +
/// styled body). `None` when the issue has no comments, so `view_detail`
/// renders no Comments panel at all.
fn comments_panel(issue: &Issue, width: u16) -> Option<Vec<Line<'static>>> {
    if issue.comments.is_empty() {
        return None;
    }
    let inner_width = panel::inner_content_width(width);
    let mut body = Vec::new();
    for (i, comment) in issue.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::from(""));
        }
        body.extend(comment_card(comment, inner_width));
    }
    let label = format!("{} ({})", t("Comments"), issue.comments.len());
    Some(panel::panel_box(&label, body, width))
}

/// One nested comment card: an unlabeled `panel::panel_box` whose body is the
/// `[author] created` header line followed by the styled, width-wrapped ADF
/// body.
fn comment_card(comment: &IssueComment, width: u16) -> Vec<Line<'static>> {
    let inner_width = panel::inner_content_width(width);
    let mut body = vec![comment_header_line(comment)];

    let mut link_occurrence = 0usize;
    let rich_body = adf_to_rich(&comment.body);
    body.extend(
        rich_body
            .iter()
            .map(|line| rich_line_to_ratatui(line, None, &mut link_occurrence)),
    );

    panel::panel_box("", wrap_lines_to_width(body, inner_width), width)
}

fn comment_header_line(comment: &IssueComment) -> Line<'static> {
    let author = comment
        .author
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| t("Unknown"));
    let created = comment.created.as_deref().unwrap_or("");
    Line::from(format!("[{author}] {created}"))
}

/// Word-agnostic display-width wrap: splits each line into as many lines as
/// needed so none exceeds `width` columns, preserving span styling across the
/// break. A zero `width` degrades to one line per input line (no infinite
/// loop).
fn wrap_lines_to_width(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    lines
        .iter()
        .flat_map(|line| wrap_line_to_width(line, width))
        .collect()
}

fn wrap_line_to_width(line: &Line<'static>, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    let mut result = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for span in &line.spans {
        let mut remaining: &str = span.content.as_ref();
        while !remaining.is_empty() {
            if current_width >= width {
                result.push(Line::from(std::mem::take(&mut current)));
                current_width = 0;
            }
            let (chunk, rest) = panel::split_at_width(remaining, (width - current_width) as u16);
            if chunk.is_empty() {
                break;
            }
            current_width += UnicodeWidthStr::width(chunk);
            current.push(Span::styled(chunk.to_owned(), span.style));
            remaining = rest;
        }
    }
    if !current.is_empty() || result.is_empty() {
        result.push(Line::from(current));
    }
    result
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
