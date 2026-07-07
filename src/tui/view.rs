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

use super::model::{footer_mode, header_line, FooterMode, Model, Screen, StatusKind, StatusMsg};
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
/// screen's reserved top row, themed via `theme::header_bar()`; while
/// `model.revalidating` the dim "refreshing…" indicator (BDR 0008 S8) is
/// overlaid on the row's right side without disturbing the identity text.
fn render_header(frame: &mut Frame, chunk: Rect, model: &Model) {
    let header = Paragraph::new(header_line(&model.identities)).style(theme::header_bar());
    frame.render_widget(header, chunk);

    if model.revalidating {
        render_refreshing_indicator(frame, chunk);
    }
}

/// Renders "refreshing…" right-aligned within a narrow slice of the header
/// row's right side (BDR 0008 S8), so the identity text on the left survives.
fn render_refreshing_indicator(frame: &mut Frame, chunk: Rect) {
    let label = t("refreshing…");
    let width = (UnicodeWidthStr::width(label.as_str()) as u16).min(chunk.width);
    let area = Rect {
        x: chunk.x + (chunk.width - width),
        y: chunk.y,
        width,
        height: chunk.height,
    };
    let indicator = Paragraph::new(label).style(theme::header_refreshing());
    frame.render_widget(indicator, area);
}

fn view_list(model: &Model, frame: &mut Frame) {
    let area = frame.area();

    let has_search_bar = model.search.is_some();
    let has_error_banner = model.error.is_some();
    let has_status_row = model.status.is_some();

    let chunks = list_layout_chunks(area, has_search_bar, has_error_banner, has_status_row);
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
    chunk_idx += 1;

    if let Some(status) = &model.status {
        render_status_row(frame, chunks[chunk_idx], status);
        chunk_idx += 1;
    }

    let hint = Paragraph::new(view_footer_text(model))
        .alignment(Alignment::Center)
        .style(theme::footer());
    frame.render_widget(hint, chunks[chunk_idx]);
}

/// Builds the `view_list` vertical layout: the header occupies the fixed top
/// row; the optional search bar, error banner, and status rows are only
/// reserved when active, sandwiched between the header and the
/// cards/footer pair.
fn list_layout_chunks(
    area: Rect,
    has_search_bar: bool,
    has_error_banner: bool,
    has_status_row: bool,
) -> std::rc::Rc<[Rect]> {
    let mut constraints = vec![Constraint::Length(1)];
    if has_search_bar {
        constraints.push(Constraint::Length(1));
    }
    if has_error_banner {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0));
    if has_status_row {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
}

/// Renders the thin transient status row above the footer (BDR 0007 S8):
/// Info in the footer's default steel style, Error in `theme::status_error()`.
fn render_status_row(frame: &mut Frame, chunk: Rect, status: &StatusMsg) {
    let style = match status.kind {
        StatusKind::Info => theme::footer(),
        StatusKind::Error => theme::status_error(),
    };
    let row = Paragraph::new(status.text.clone())
        .alignment(Alignment::Center)
        .style(style);
    frame.render_widget(row, chunk);
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

/// Resolves a left click at absolute terminal row `y` within the list
/// screen's full frame `area` to the clicked card's row index (ADR 0017 §3,
/// BDR 0009 S3-S5): built on the exact `list_layout_chunks`/
/// `first_visible_card`/`CARD_HEIGHT` the renderer uses, so hit-testing can
/// never drift from what's drawn. `None` when the click lands outside the
/// cards chunk (header/footer/status rows), past the last visible card, or
/// the list is empty.
pub(super) fn list_click_card(model: &Model, area: Rect, y: u16) -> Option<usize> {
    if model.rows.is_empty() {
        return None;
    }

    let has_search_bar = model.search.is_some();
    let has_error_banner = model.error.is_some();
    let has_status_row = model.status.is_some();
    let chunks = list_layout_chunks(area, has_search_bar, has_error_banner, has_status_row);
    // Mirrors `view_list`'s own chunk order: header, [search], [error], cards, ...
    let cards_idx = 1 + usize::from(has_search_bar) + usize::from(has_error_banner);
    let chunk = chunks[cards_idx];

    if y < chunk.y || y >= chunk.y + chunk.height {
        return None;
    }

    let visible = (chunk.height / CARD_HEIGHT).max(1) as usize;
    let slot = ((y - chunk.y) / CARD_HEIGHT) as usize;
    if slot >= visible {
        return None;
    }

    let first = first_visible_card(model.selected, model.rows.len(), visible);
    let candidate = first + slot;
    (candidate < model.rows.len()).then_some(candidate)
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

/// The single mode-aware footer hint source (ADR 0014 §5, BDR 0007 S7): every
/// hint text routes through `t()`, one string per [`FooterMode`], no
/// per-screen branching outside this function.
pub(crate) fn footer_hint(mode: FooterMode) -> String {
    match mode {
        FooterMode::List => t("↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"),
        FooterMode::ListSearch => t("Enter submit  Esc cancel  Backspace delete"),
        FooterMode::Detail => t("↑/↓ j/k scroll  Esc/b back  q quit"),
        FooterMode::DetailLink => {
            t("↑/↓ j/k scroll  Tab next link  Enter open  Esc/b back  q quit")
        }
    }
}

/// The rendered footer text for the current model: `footer_hint` for the
/// active mode, with the load-more affordance appended only in plain List
/// mode while a paging cursor is pending (P3, unchanged behavior).
fn view_footer_text(model: &Model) -> String {
    let mode = footer_mode(model);
    let hint = footer_hint(mode);
    if mode == FooterMode::List && model.next_page_token.is_some() {
        format!("{hint}  {}", t("n more"))
    } else {
        hint
    }
}

/// The frame border + the scroll-content region's top/bottom rows (BDR 0007
/// S6 clamp math needs the same figure the renderer subtracts).
const DETAIL_FRAME_BORDER_ROWS: u16 = 2;
const DETAIL_FRAME_BORDER_COLS: u16 = 2;

/// Builds the `view_detail` vertical layout: the header occupies the fixed
/// top row, the content region takes the remaining space, the optional
/// status row is only reserved when active, and the footer is the fixed
/// bottom row (mirrors `list_layout_chunks`'s pattern) — so `detail_link_at`
/// can recompute the exact content chunk `render_detail_panels` draws into
/// (ADR 0018 §5, single geometry source).
fn detail_layout_chunks(area: Rect, has_status_row: bool) -> std::rc::Rc<[Rect]> {
    let mut constraints = vec![Constraint::Length(1), Constraint::Min(0)];
    if has_status_row {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
}

/// Pure detail view — renders the loaded issue or a loading notice.
pub fn view_detail(model: &Model, frame: &mut Frame) {
    let area = frame.area();
    let has_status_row = model.status.is_some();
    let chunks = detail_layout_chunks(area, has_status_row);

    render_header(frame, chunks[0], model);

    let footer_idx = chunks.len() - 1;
    let footer = Paragraph::new(footer_hint(footer_mode(model)))
        .alignment(Alignment::Center)
        .style(theme::footer());
    frame.render_widget(footer, chunks[footer_idx]);

    if let Some(status) = &model.status {
        render_status_row(frame, chunks[footer_idx - 1], status);
    }

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
    let lines = compose_detail(issue, focused_link, inner_width).lines;
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

/// One href-carrying cell in a composed row's cursor-hit metadata (ADR 0018
/// §5): its display width and the href of the span it belongs to (`None` for
/// plain body text, borders, and padding). `detail_link_at` walks a row's
/// cells by width to find the one under the click column; the renderer never
/// reads this.
#[derive(Clone)]
struct LinkCell {
    width: u16,
    href: Option<String>,
}

/// The single detail compose result (ADR 0018 §5): the ratatui `Line`s the
/// renderer draws, and — row for row — the href hit-test metadata
/// `detail_link_at` walks. Both come out of the same pass over the same
/// wrapped content, so they cannot drift apart (no duplicated wrap/border
/// math).
struct DetailCompose {
    lines: Vec<Line<'static>>,
    link_rows: Vec<Vec<LinkCell>>,
}

/// Composes the single globally-scrolled line buffer (BDR 0007 S5) alongside
/// its href hit-test metadata, row for row (ADR 0018 §5): the Details meta
/// panel, the Description panel, and — when present — the Comments panel,
/// each drawn via `panel::panel_box` at the same `width`. The Details and
/// Comments panels carry no hrefs (empty rows); only the Description panel's
/// inline `[url]` tokens do.
fn compose_detail(issue: &Issue, focused_link: Option<usize>, width: u16) -> DetailCompose {
    let details_lines = details_panel(issue, width);
    let details_link_rows = vec![Vec::new(); details_lines.len()];

    let (description_lines, description_link_rows) =
        description_panel_compose(issue, focused_link, width);

    let mut lines = details_lines;
    lines.push(Line::from(""));
    lines.extend(description_lines);

    let mut link_rows = details_link_rows;
    link_rows.push(Vec::new());
    link_rows.extend(description_link_rows);

    if let Some(comments_lines) = comments_panel(issue, width) {
        let comments_link_rows = vec![Vec::new(); comments_lines.len()];
        lines.push(Line::from(""));
        lines.extend(comments_lines);
        link_rows.push(Vec::new());
        link_rows.extend(comments_link_rows);
    }

    DetailCompose { lines, link_rows }
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

/// One rendered text run's ratatui style alongside its href (`None` off the
/// visible `[url]` token) — the shared unit `wrap_run_line_to_width` carries
/// through wrapping, so the renderer's `Line`s and `detail_link_at`'s
/// hit-test cells walk the identical width-driven wrap decision (ADR 0018
/// §5, single geometry source).
#[derive(Clone)]
struct SpanRun {
    text: String,
    style: Style,
    href: Option<String>,
}

/// A wrapped/unwrapped line of [`SpanRun`]s — the run-carrying counterpart of
/// a ratatui `Line` before hrefs are dropped for rendering.
type RunLine = Vec<SpanRun>;

fn run_line_to_line(run_line: &RunLine) -> Line<'static> {
    Line::from(
        run_line
            .iter()
            .map(|run| Span::styled(run.text.clone(), run.style))
            .collect::<Vec<_>>(),
    )
}

fn run_lines_to_lines(run_lines: &[RunLine]) -> Vec<Line<'static>> {
    run_lines.iter().map(run_line_to_line).collect()
}

/// Lifts an already-built ratatui `Line` (no href) into a [`RunLine`], so
/// plain content (e.g. a comment's header line) can be wrapped through the
/// same run-based pipeline as href-carrying ADF content.
fn line_to_run_line(line: &Line<'static>) -> RunLine {
    line.spans
        .iter()
        .map(|span| SpanRun {
            text: span.content.to_string(),
            style: span.style,
            href: None,
        })
        .collect()
}

/// The panel-content left offset a boxed row's rendered `Line` starts with
/// (`panel::panel_box`'s `"│ "`: one border column, one padding column) —
/// `detail_link_at`'s column walk must skip exactly this many columns before
/// reaching the first content cell.
const PANEL_CONTENT_LEFT_OFFSET: u16 = 2;

fn run_line_to_link_cells(run_line: &RunLine) -> Vec<LinkCell> {
    run_line
        .iter()
        .map(|run| LinkCell {
            width: UnicodeWidthStr::width(run.text.as_str()) as u16,
            href: run.href.clone(),
        })
        .collect()
}

/// Wraps a boxed panel's content link-rows with the border/padding row and
/// per-row left-offset cell `panel::panel_box` renders around the content
/// (ADR 0018 §5): a borderless row for the top border, one leading no-href
/// cell per content row for `"│ "`, and a borderless row for the bottom
/// border.
fn box_link_rows(content_rows: Vec<Vec<LinkCell>>) -> Vec<Vec<LinkCell>> {
    let mut rows = Vec::with_capacity(content_rows.len() + 2);
    rows.push(Vec::new());
    for row in content_rows {
        let mut boxed_row = Vec::with_capacity(row.len() + 1);
        boxed_row.push(LinkCell {
            width: PANEL_CONTENT_LEFT_OFFSET,
            href: None,
        });
        boxed_row.extend(row);
        rows.push(boxed_row);
    }
    rows.push(Vec::new());
    rows
}

/// The Description panel (BDR 0007 S5) plus its href hit-test metadata (ADR
/// 0018 §5): the styled ADF run-lines, wrapped to the panel's inner width
/// before boxing so no line exceeds it (offset math stays exact),
/// focused-link `REVERSED` styling survives, and every wrapped fragment of a
/// `[url]` token keeps the complete href (BDR 0010 S7).
fn description_panel_compose(
    issue: &Issue,
    focused_link: Option<usize>,
    width: u16,
) -> (Vec<Line<'static>>, Vec<Vec<LinkCell>>) {
    let description = issue
        .description
        .as_deref()
        .map(adf_to_rich)
        .unwrap_or_default();
    let inner_width = panel::inner_content_width(width);
    let styled = description_lines_to_runs(&description, focused_link);
    let wrapped = wrap_run_lines_to_width(styled, inner_width);

    let lines = panel::panel_box(&t("Description"), run_lines_to_lines(&wrapped), width);
    let content_link_rows: Vec<Vec<LinkCell>> =
        wrapped.iter().map(run_line_to_link_cells).collect();
    let link_rows = box_link_rows(content_link_rows);

    (lines, link_rows)
}

/// The Comments panel (BDR 0007 S5): titled `Comments (N)`, its body made of
/// nested per-comment cards (empty label, author `[created]` header line +
/// styled body). `None` when the issue has no comments, so `view_detail`
/// renders no Comments panel at all. Comment bodies carry no href hit-test
/// metadata (out of BDR 0010's scope — only the Description panel's tokens
/// are click-activated this slice); a modifier-click over one safely
/// resolves to `None`.
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
    let mut body = vec![line_to_run_line(&comment_header_line(comment))];

    let mut link_occurrence = 0usize;
    let rich_body = adf_to_rich(&comment.body);
    body.extend(
        rich_body
            .iter()
            .map(|line| rich_line_to_runs(line, None, &mut link_occurrence)),
    );

    let wrapped = wrap_run_lines_to_width(body, inner_width);
    panel::panel_box("", run_lines_to_lines(&wrapped), width)
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
/// needed so none exceeds `width` columns, preserving each run's style and
/// href across the break (BDR 0010 S7: a wrapped `[url]` fragment keeps the
/// complete href). A zero `width` degrades to one line per input line (no
/// infinite loop).
fn wrap_run_lines_to_width(lines: Vec<RunLine>, width: u16) -> Vec<RunLine> {
    lines
        .iter()
        .flat_map(|line| wrap_run_line_to_width(line, width))
        .collect()
}

fn wrap_run_line_to_width(line: &RunLine, width: u16) -> Vec<RunLine> {
    let width = width.max(1) as usize;
    let mut result = Vec::new();
    let mut current: RunLine = Vec::new();
    let mut current_width = 0usize;

    for run in line {
        let mut remaining: &str = run.text.as_str();
        while !remaining.is_empty() {
            if current_width >= width {
                result.push(std::mem::take(&mut current));
                current_width = 0;
            }
            let (chunk, rest) = panel::split_at_width(remaining, (width - current_width) as u16);
            if chunk.is_empty() {
                break;
            }
            current_width += UnicodeWidthStr::width(chunk);
            current.push(SpanRun {
                text: chunk.to_owned(),
                style: run.style,
                href: run.href.clone(),
            });
            remaining = rest;
        }
    }
    if !current.is_empty() || result.is_empty() {
        result.push(current);
    }
    result
}

/// Maps a rich-render style to its ratatui equivalent (ADR 0014 §1, ADR 0018
/// §6): an href-carrying run additionally takes the theme link color (in
/// addition to its own modifiers) so the `[url]` token renders visibly
/// link-styled; anchor text (no href) stays body-colored.
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
    if style.link.is_some() {
        result = result.patch(theme::link());
    }
    result
}

/// Maps the description's rich lines to run-lines (style + href), reversing
/// the style of the inline link whose render-order occurrence matches
/// `focused_link`.
fn description_lines_to_runs(
    description: &[RichLine],
    focused_link: Option<usize>,
) -> Vec<RunLine> {
    let mut link_occurrence = 0usize;
    description
        .iter()
        .map(|line| rich_line_to_runs(line, focused_link, &mut link_occurrence))
        .collect()
}

fn rich_line_to_runs(
    line: &RichLine,
    focused_link: Option<usize>,
    link_occurrence: &mut usize,
) -> RunLine {
    line.iter()
        .map(|span| {
            let style = span_style(span, focused_link, link_occurrence);
            SpanRun {
                text: span.text.clone(),
                style,
                href: span.style.link.clone(),
            }
        })
        .collect()
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

/// Resolves a modifier-click at absolute terminal `(x, y)` within the Detail
/// screen's full frame `area` to the href of the span under the cursor (ADR
/// 0018 §5, BDR 0010 S5/S7/S8): recomputes `compose_detail`'s own
/// wrap/scroll/panel-chrome pipeline — the exact one `render_detail_panels`
/// draws — so the hit test can never drift from what is on screen. `None`
/// when there is no loaded issue, the coordinate falls outside the content
/// viewport, lands on chrome (borders/padding/blank rows), or the span under
/// the cursor carries no href.
pub(super) fn detail_link_at(model: &Model, area: Rect, x: u16, y: u16) -> Option<String> {
    let issue = model.detail.as_ref()?;
    let has_status_row = model.status.is_some();
    let content_area = detail_layout_chunks(area, has_status_row)[1];

    let inner_width = content_area.width.saturating_sub(DETAIL_FRAME_BORDER_COLS);
    let compose = compose_detail(issue, model.detail_focused_link, inner_width);

    let viewport_height = content_area.height.saturating_sub(DETAIL_FRAME_BORDER_ROWS);
    let total_lines = compose.lines.len() as u16;
    let offset = clamp_scroll_offset(model.detail_scroll, total_lines, viewport_height);

    let content_top = content_area.y + 1;
    let content_left = content_area.x + 1;
    if y < content_top || x < content_left {
        return None;
    }
    let row_in_viewport = y - content_top;
    if row_in_viewport >= viewport_height {
        return None;
    }

    let row_index = (offset + row_in_viewport) as usize;
    let col = x - content_left;
    let row = compose.link_rows.get(row_index)?;
    cell_at_column(row, col)
}

/// Walks a composed row's href-carrying cells by display width
/// (unicode-width, consistent with `panel.rs`) to find the cell under
/// column `col`. `None` when `col` falls past the row's content or lands on
/// a cell carrying no href.
fn cell_at_column(row: &[LinkCell], col: u16) -> Option<String> {
    let mut used = 0u16;
    for cell in row {
        if col < used + cell.width {
            return cell.href.clone();
        }
        used += cell.width;
    }
    None
}
