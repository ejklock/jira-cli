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

use super::modal;
use super::model::{
    footer_mode, header_line, Compose, ComposeStatus, FooterMode, Model, Screen, Selection,
    StatusKind, StatusMsg,
};
use super::panel;
use super::theme;
use crate::i18n::t;
use crate::models::{Attachment, Issue, IssueComment, IssueRow, ProjectRow};
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
        Screen::Projects => view_projects(model, frame),
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
        FooterMode::Projects => t("↑/↓ navigate  Enter select  Esc/b back  q quit"),
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

/// The Projects screen's row height — one line per `KEY — name` row (ADR
/// 0021, BDR 0013 S1-S2), plainer than the List screen's bordered cards.
const PROJECT_ROW_HEIGHT: u16 = 1;

/// The Projects screen's title line height, reserved above its rows.
const PROJECTS_TITLE_HEIGHT: u16 = 1;

/// Builds the `view_projects` vertical layout: the header occupies the fixed
/// top row; the content region takes the remaining space; the optional
/// status row is only reserved when active; the footer is the fixed bottom
/// row — mirrors `detail_layout_chunks`'s shape (no search/error banners on
/// this screen) so `projects_click_row` can recompute the exact content
/// chunk `view_projects` draws into (single layout source, ADR 0017).
fn projects_layout_chunks(area: Rect, has_status_row: bool) -> std::rc::Rc<[Rect]> {
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

/// The content chunk's rows sub-region, below the reserved title line — the
/// same split `render_projects_rows` and `projects_click_row` both read.
fn projects_rows_chunk(chunk: Rect) -> Rect {
    let title_height = PROJECTS_TITLE_HEIGHT.min(chunk.height);
    Rect {
        x: chunk.x,
        y: chunk.y + title_height,
        width: chunk.width,
        height: chunk.height.saturating_sub(title_height),
    }
}

/// The Projects screen (ADR 0021, BDR 0013 S1-S5): themed header, a
/// localized "Projects" title, one `KEY — name` row per project (the
/// selected row styled like the list's selected card), a localized
/// empty-state notice when there are no projects, the status row (fetch
/// error or the loading indicator), and the mode-aware footer hint.
fn view_projects(model: &Model, frame: &mut Frame) {
    let area = frame.area();
    let has_status_row = model.status.is_some();
    let chunks = projects_layout_chunks(area, has_status_row);

    render_header(frame, chunks[0], model);
    render_projects_rows(frame, chunks[1], model);

    if let Some(status) = &model.status {
        render_status_row(frame, chunks[2], status);
    }

    let footer_idx = chunks.len() - 1;
    let hint = Paragraph::new(footer_hint(footer_mode(model)))
        .alignment(Alignment::Center)
        .style(theme::footer());
    frame.render_widget(hint, chunks[footer_idx]);
}

fn render_projects_title(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(t("Projects")).style(theme::section_title());
    frame.render_widget(title, area);
}

/// Renders the Projects screen's content region: the title line, then either
/// the localized empty-state notice or the windowed `KEY — name` rows (BDR
/// 0013 S1-S2), keeping the selected row in view exactly like the list's
/// card window (`first_visible_card` — single windowing source).
fn render_projects_rows(frame: &mut Frame, chunk: Rect, model: &Model) {
    let title_height = PROJECTS_TITLE_HEIGHT.min(chunk.height);
    render_projects_title(
        frame,
        Rect {
            x: chunk.x,
            y: chunk.y,
            width: chunk.width,
            height: title_height,
        },
    );

    let rows_chunk = projects_rows_chunk(chunk);
    if model.projects.is_empty() {
        let notice = Paragraph::new(t("No projects.")).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        frame.render_widget(notice, rows_chunk);
        return;
    }

    let visible = rows_chunk.height.max(1) as usize;
    let first = first_visible_card(model.projects_selected, model.projects.len(), visible);
    let last = (first + visible).min(model.projects.len());

    for (offset, project) in model.projects[first..last].iter().enumerate() {
        let index = first + offset;
        let row_area = Rect {
            x: rows_chunk.x,
            y: rows_chunk.y + offset as u16,
            width: rows_chunk.width,
            height: PROJECT_ROW_HEIGHT,
        };
        if row_area.y + PROJECT_ROW_HEIGHT > rows_chunk.y + rows_chunk.height {
            break;
        }
        render_project_row(frame, row_area, project, index == model.projects_selected);
    }
}

/// One `KEY — name` row; the selected row carries `theme::selected()` (the
/// same style the list's selected card uses).
fn render_project_row(frame: &mut Frame, area: Rect, project: &ProjectRow, is_selected: bool) {
    let style = is_selected.then(theme::selected).unwrap_or_default();
    let text = format!("{} — {}", project.key, project.name);
    let paragraph = Paragraph::new(text).style(style);
    frame.render_widget(paragraph, area);
}

/// Resolves a left click at absolute terminal row `y` within the Projects
/// screen's full frame `area` to the clicked row's index (ADR 0021, BDR 0013
/// S2-S3): built on the exact `projects_layout_chunks`/`projects_rows_chunk`/
/// `first_visible_card` the renderer uses (single layout source, ADR 0017),
/// mirroring `list_click_card`'s contract. `None` when the click lands
/// outside the rows chunk, past the last visible row, or the list is empty.
pub(super) fn projects_click_row(model: &Model, area: Rect, y: u16) -> Option<usize> {
    if model.projects.is_empty() {
        return None;
    }

    let has_status_row = model.status.is_some();
    let chunks = projects_layout_chunks(area, has_status_row);
    let rows_chunk = projects_rows_chunk(chunks[1]);

    if y < rows_chunk.y || y >= rows_chunk.y + rows_chunk.height {
        return None;
    }

    let visible = rows_chunk.height.max(1) as usize;
    let slot = (y - rows_chunk.y) as usize;
    if slot >= visible {
        return None;
    }

    let first = first_visible_card(model.projects_selected, model.projects.len(), visible);
    let candidate = first + slot;
    (candidate < model.projects.len()).then_some(candidate)
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
            model.detail_focused_comment,
            model.detail_scroll,
            model.selection.as_ref(),
        ),
    }

    if let Some(compose) = &model.compose {
        render_compose_modal(frame, area, compose);
    }
    if model.confirm.is_some() {
        modal::render_modal(frame, area, &confirm_modal_content());
    }
}

/// Renders the comment compose over the detail through the C3a modal
/// primitive (ADR 0024 §3, ADR 0026 §3, BDR 0015 S5, BDR 0017 S3): every
/// user-facing string routes through `t()`, the title comes from the
/// compose's `target` (`ComposeTarget::title_key`, "New comment" vs "Edit
/// comment"), the buffer becomes the modal's body split on `\n` (Enter's
/// newline, S1), and the status line reflects `ComposeStatus`. Only ever
/// called from `view_detail` with `model.compose` set, so it never renders
/// on List/Projects.
fn render_compose_modal(frame: &mut Frame, area: Rect, compose: &Compose) {
    let content = modal::ModalContent {
        title: t(compose.target.title_key()),
        body: compose_body_lines(&compose.buffer),
        hint: Some(t("Ctrl+S send · Esc cancel")),
        status: compose_status_text(&compose.status),
        buttons: vec![],
    };
    modal::render_modal(frame, area, &content);
}

fn compose_body_lines(buffer: &str) -> Vec<Line<'static>> {
    buffer
        .split('\n')
        .map(|line| Line::from(line.to_owned()))
        .collect()
}

fn compose_status_text(status: &ComposeStatus) -> Option<String> {
    match status {
        ComposeStatus::Idle => None,
        ComposeStatus::Submitting => Some(t("Sending…")),
        ComposeStatus::Error(reason) => Some(reason.clone()),
    }
}

/// The delete-confirm modal's content (ADR 0026 §4, BDR 0017 S7, S10): a
/// fixed localized prompt plus Sim/Não buttons. Built entirely in `view.rs`
/// — mirroring `render_compose_modal`'s own in-view `ModalContent`
/// construction — rather than as a `Model` method, so `model.rs` stays free
/// of ratatui types (`ModalContent::body` is `Vec<Line<'static>>`), matching
/// the documented pure-core boundary (ADR 0007 §6). Pure — no rendering, no
/// I/O — so it is headlessly unit-tested.
pub(super) fn confirm_modal_content() -> modal::ModalContent {
    modal::ModalContent {
        title: t("Delete comment?"),
        body: vec![Line::from(t("This action cannot be undone."))],
        hint: Some(t("y/Enter confirm · n/Esc cancel")),
        status: None,
        buttons: vec![
            modal::ModalButton {
                id: "yes".to_owned(),
                label: t("Yes"),
            },
            modal::ModalButton {
                id: "no".to_owned(),
                label: t("No"),
            },
        ],
    }
}

/// Renders the detail frame (ADR 0014 §4, BDR 0007 S5-S6): the issue summary
/// as the border title, the Details/Description/Comments panels as one
/// globally-scrolled `Paragraph`, and a `Scrollbar` when content overflows
/// the viewport. The offset clamps to the content's last page so scrolling
/// past the end never leaves blank overscroll. An active `selection` (ADR
/// 0019 §5) is patched onto the covered spans before scrolling is applied,
/// so the highlight scrolls with the content.
fn render_detail_panels(
    frame: &mut Frame,
    area: Rect,
    issue: &Issue,
    focused_link: Option<usize>,
    focused_comment: Option<usize>,
    scroll: u16,
    selection: Option<&Selection>,
) {
    let inner_width = area.width.saturating_sub(DETAIL_FRAME_BORDER_COLS);
    let compose = compose_detail(issue, focused_link, focused_comment, inner_width);
    let lines = apply_selection_highlight(compose, selection);
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

/// A cell's logical provenance (ADR 0019 §1): which pre-wrap logical line it
/// came from and its char range (CHAR indices, never bytes/display columns)
/// within that line's text. `None` on the padding/border cells `box_link_rows`
/// inserts around content — chrome carries no logical position, so it can
/// never be selected or copied (BDR 0011 S5).
#[derive(Clone, Copy)]
struct CellSpan {
    logical_line: usize,
    char_start: usize,
    char_len: usize,
}

/// One rendered cell in a composed row's hit-test metadata (ADR 0018 §5, ADR
/// 0019 §1): its display width, the href of the span it belongs to (`None`
/// for plain body text, borders, and padding), and its logical provenance.
/// `detail_link_at`/`detail_pos_at` walk a row's cells by width to find the
/// one under a column; the renderer never reads this.
#[derive(Clone)]
struct LinkCell {
    width: u16,
    href: Option<String>,
    span: Option<CellSpan>,
}

/// The single detail compose result (ADR 0018 §5, ADR 0019 §1): the ratatui
/// `Line`s the renderer draws; row for row, the hit-test metadata every
/// pointer resolver walks; and the chrome-free pre-wrap text of each logical
/// line, indexed by `CellSpan::logical_line`. All three come out of the same
/// pass over the same wrapped content, so they cannot drift apart (no
/// duplicated wrap/border math).
struct DetailCompose {
    lines: Vec<Line<'static>>,
    link_rows: Vec<Vec<LinkCell>>,
    logical_lines: Vec<String>,
}

/// Composes the single globally-scrolled line buffer (BDR 0007 S5) alongside
/// its hit-test metadata, row for row (ADR 0018 §5, ADR 0019 §1): the Details
/// meta panel, the Description panel, and — when present — the Comments and
/// Attachments panels, each drawn via `panel::panel_box` at the same `width`.
/// The Description panel's inline `[url]` tokens and the Attachments panel's
/// `[n] ↗ filename` rows carry an `href` (ADR 0020); every content cell
/// across all panels carries logical provenance for selection.
fn compose_detail(
    issue: &Issue,
    focused_link: Option<usize>,
    focused_comment: Option<usize>,
    width: u16,
) -> DetailCompose {
    let mut logical_lines = Vec::new();

    let (details_lines, details_link_rows) = details_panel(issue, width, &mut logical_lines);

    let (description_lines, description_link_rows) =
        description_panel_compose(issue, focused_link, width, &mut logical_lines);

    let mut lines = details_lines;
    lines.push(Line::from(""));
    lines.extend(description_lines);

    let mut link_rows = details_link_rows;
    link_rows.push(Vec::new());
    link_rows.extend(description_link_rows);

    if let Some((comments_lines, comments_link_rows)) =
        comments_panel(issue, width, focused_comment, &mut logical_lines)
    {
        lines.push(Line::from(""));
        lines.extend(comments_lines);
        link_rows.push(Vec::new());
        link_rows.extend(comments_link_rows);
    }

    if let Some((attachments_lines, attachments_link_rows)) =
        attachments_panel(issue, width, &mut logical_lines)
    {
        lines.push(Line::from(""));
        lines.extend(attachments_lines);
        link_rows.push(Vec::new());
        link_rows.extend(attachments_link_rows);
    }

    DetailCompose {
        lines,
        link_rows,
        logical_lines,
    }
}

/// Registers `text` as the next logical line and returns its index — the
/// single seam every panel uses so `logical_lines` and each `SpanRun`'s
/// `logical_line` index can never drift apart (ADR 0019 §1).
fn register_logical_line(logical_lines: &mut Vec<String>, text: String) -> usize {
    let idx = logical_lines.len();
    logical_lines.push(text);
    idx
}

/// The Details panel (BDR 0007 S5): a 2-column meta table — Title, Key,
/// Status, Type, Assignee, Created, Updated, and Due (each of the latter
/// three omitted when absent/unparseable). Also produces the panel's
/// hit-test metadata (ADR 0019 §1) so its rows are selectable and copyable
/// like every other panel.
fn details_panel(
    issue: &Issue,
    width: u16,
    logical_lines: &mut Vec<String>,
) -> (Vec<Line<'static>>, Vec<Vec<LinkCell>>) {
    let inner_width = panel::inner_content_width(width);
    let rows = details_meta_run_lines(issue, inner_width, logical_lines);
    let wrapped = wrap_run_lines_to_width(rows, inner_width);
    let lines = panel::panel_box(&t("Details"), run_lines_to_lines(&wrapped), width);
    let content_link_rows: Vec<Vec<LinkCell>> =
        wrapped.iter().map(run_line_to_link_cells).collect();
    let link_rows = box_link_rows(content_link_rows);
    (lines, link_rows)
}

fn details_meta_run_lines(
    issue: &Issue,
    inner_width: u16,
    logical_lines: &mut Vec<String>,
) -> Vec<RunLine> {
    let mut rows = vec![
        meta_run_line(&t("Title"), &issue.summary, inner_width, logical_lines),
        meta_run_line(&t("Key"), &issue.key, inner_width, logical_lines),
        meta_run_line(
            &t("Status"),
            &status_text(issue),
            inner_width,
            logical_lines,
        ),
        meta_run_line(&t("Type"), &issue.issue_type, inner_width, logical_lines),
        meta_run_line(
            &t("Assignee"),
            &assignee_text(issue),
            inner_width,
            logical_lines,
        ),
    ];
    if let Some(created) = &issue.created {
        rows.push(meta_run_line(
            &t("Created"),
            created,
            inner_width,
            logical_lines,
        ));
    }
    if let Some(updated) = &issue.updated {
        rows.push(meta_run_line(
            &t("Updated"),
            updated,
            inner_width,
            logical_lines,
        ));
    }
    if let Some(due) = due_relative_text(issue) {
        rows.push(meta_run_line(&t("Due"), &due, inner_width, logical_lines));
    }
    rows
}

fn meta_run_line(
    label: &str,
    value: &str,
    inner_width: u16,
    logical_lines: &mut Vec<String>,
) -> RunLine {
    let prefix = format!("{label}: ");
    let budget = inner_width.saturating_sub(UnicodeWidthStr::width(prefix.as_str()) as u16);
    let value = panel::ellipsize_display(value, budget);
    let text = format!("{prefix}{value}");
    let logical_line = register_logical_line(logical_lines, text.clone());
    vec![SpanRun {
        text,
        style: Style::default(),
        href: None,
        logical_line,
        char_start: 0,
    }]
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

/// One rendered text run's ratatui style, its href (`None` off the visible
/// `[url]` token), and its logical provenance (ADR 0019 §1: which pre-wrap
/// logical line it came from and its CHAR offset within that line's text) —
/// the shared unit `wrap_run_line_to_width` carries through wrapping, so the
/// renderer's `Line`s, `detail_link_at`'s hit-test cells, and every selection
/// resolver walk the identical width-driven wrap decision (ADR 0018 §5, ADR
/// 0019 §2, single geometry source).
#[derive(Clone)]
struct SpanRun {
    text: String,
    style: Style,
    href: Option<String>,
    logical_line: usize,
    char_start: usize,
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
/// same run-based pipeline as href-carrying ADF content. Registers the
/// line's full text as one logical line (ADR 0019 §1), with each span's char
/// offset accumulated across the line's spans.
fn line_to_run_line(line: &Line<'static>, logical_lines: &mut Vec<String>) -> RunLine {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    let logical_line = register_logical_line(logical_lines, text);
    let mut char_start = 0usize;
    line.spans
        .iter()
        .map(|span| {
            let text = span.content.to_string();
            let char_len = text.chars().count();
            let run = SpanRun {
                text,
                style: span.style,
                href: None,
                logical_line,
                char_start,
            };
            char_start += char_len;
            run
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
            span: Some(CellSpan {
                logical_line: run.logical_line,
                char_start: run.char_start,
                char_len: run.text.chars().count(),
            }),
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
            span: None,
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
    logical_lines: &mut Vec<String>,
) -> (Vec<Line<'static>>, Vec<Vec<LinkCell>>) {
    let description = issue
        .description
        .as_deref()
        .map(adf_to_rich)
        .unwrap_or_default();
    let inner_width = panel::inner_content_width(width);
    let styled = description_lines_to_runs(&description, focused_link, logical_lines);
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
/// renders no Comments panel at all. Comment bodies carry logical provenance
/// for selection (ADR 0019 §1) but no href hit-test metadata (out of BDR
/// 0010's scope — only the Description panel's tokens are click-activated
/// this slice); a modifier-click over one safely resolves to `None`. The
/// comment at `focused_comment`'s index renders with the focused-comment
/// highlight (ADR 0026 §1, BDR 0017 S1), mirroring the focused-link
/// highlight.
fn comments_panel(
    issue: &Issue,
    width: u16,
    focused_comment: Option<usize>,
    logical_lines: &mut Vec<String>,
) -> Option<(Vec<Line<'static>>, Vec<Vec<LinkCell>>)> {
    if issue.comments.is_empty() {
        return None;
    }
    let inner_width = panel::inner_content_width(width);
    let mut body = Vec::new();
    let mut body_link_rows = Vec::new();
    for (i, comment) in issue.comments.iter().enumerate() {
        if i > 0 {
            body.push(Line::from(""));
            body_link_rows.push(Vec::new());
        }
        let is_focused = focused_comment == Some(i);
        let (card_lines, card_link_rows) =
            comment_card(comment, inner_width, is_focused, logical_lines);
        body.extend(card_lines);
        body_link_rows.extend(card_link_rows);
    }
    let label = format!("{} ({})", t("Comments"), issue.comments.len());
    let lines = panel::panel_box(&label, body, width);
    let link_rows = box_link_rows(body_link_rows);
    Some((lines, link_rows))
}

/// One nested comment card: an unlabeled `panel::panel_box` whose body is the
/// `[author] created` header line followed by the styled, width-wrapped ADF
/// body, plus its own hit-test metadata (ADR 0019 §1) — comment href capture
/// stays off (`capture_href: false`), matching the existing no-activation
/// contract. `is_focused` patches the focused-comment highlight (ADR 0026 §1,
/// BDR 0017 S1) onto every rendered cell of the card, mirroring
/// `theme::selection_highlight()`'s REVERSED treatment of a focused link.
fn comment_card(
    comment: &IssueComment,
    width: u16,
    is_focused: bool,
    logical_lines: &mut Vec<String>,
) -> (Vec<Line<'static>>, Vec<Vec<LinkCell>>) {
    let inner_width = panel::inner_content_width(width);
    let header_run_line = line_to_run_line(&comment_header_line(comment), logical_lines);
    let mut body = vec![header_run_line];

    let mut link_occurrence = 0usize;
    let rich_body = adf_to_rich(&comment.body);
    body.extend(
        rich_body
            .iter()
            .map(|line| rich_line_to_runs(line, None, &mut link_occurrence, logical_lines, false)),
    );

    let wrapped = wrap_run_lines_to_width(body, inner_width);
    let lines = panel::panel_box("", run_lines_to_lines(&wrapped), width);
    let lines = if is_focused {
        highlight_lines(lines)
    } else {
        lines
    };
    let content_link_rows: Vec<Vec<LinkCell>> =
        wrapped.iter().map(run_line_to_link_cells).collect();
    let link_rows = box_link_rows(content_link_rows);
    (lines, link_rows)
}

/// Patches `theme::selection_highlight()` (the same REVERSED-class style the
/// focused inline link uses) onto every span of every line, so a focused
/// comment card renders visibly regardless of its own fg/bg/border style.
fn highlight_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines.into_iter().map(highlight_line).collect()
}

fn highlight_line(line: Line<'static>) -> Line<'static> {
    let highlight = theme::selection_highlight();
    Line::from(
        line.spans
            .into_iter()
            .map(|span| Span::styled(span.content, span.style.patch(highlight)))
            .collect::<Vec<_>>(),
    )
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

/// The Attachments panel (ADR 0020, BDR 0012 S3-S8): titled `Attachments
/// (N)`, one `[n] ↗ filename` row per attachment whose WHOLE row carries the
/// theme link style and `href` = the attachment's content URL — so B2b's
/// modifier-click activation and B3's selection/extraction work over
/// attachment rows through the SAME `RunLine`/`SpanRun` pipeline every other
/// panel uses, with zero new click/selection machinery. One blank row
/// separates consecutive attachment rows (S4 breathing room); the panel's
/// last line is the italic/dim Ctrl/Cmd+click footnote. `None` when the issue
/// has no attachments, so `compose_detail` renders no Attachments panel at
/// all (S8).
fn attachments_panel(
    issue: &Issue,
    width: u16,
    logical_lines: &mut Vec<String>,
) -> Option<(Vec<Line<'static>>, Vec<Vec<LinkCell>>)> {
    if issue.attachments.is_empty() {
        return None;
    }
    let inner_width = panel::inner_content_width(width);
    let mut rows = attachment_run_lines(&issue.attachments, logical_lines);
    rows.push(attachments_footnote_run_line(logical_lines));

    let wrapped = wrap_run_lines_to_width(rows, inner_width);
    let label = format!("{} ({})", t("Attachments"), issue.attachments.len());
    let lines = panel::panel_box(&label, run_lines_to_lines(&wrapped), width);
    let content_link_rows: Vec<Vec<LinkCell>> =
        wrapped.iter().map(run_line_to_link_cells).collect();
    let link_rows = box_link_rows(content_link_rows);
    Some((lines, link_rows))
}

/// One `[n] ↗ filename` run-line per attachment (S3/S6), with one blank
/// run-line between consecutive attachments (S4).
fn attachment_run_lines(
    attachments: &[Attachment],
    logical_lines: &mut Vec<String>,
) -> Vec<RunLine> {
    let mut rows = Vec::with_capacity(attachments.len() * 2);
    for (index, attachment) in attachments.iter().enumerate() {
        if index > 0 {
            rows.push(Vec::new());
        }
        rows.push(attachment_run_line(attachment, index, logical_lines));
    }
    rows
}

/// One attachment's row: the WHOLE `[n] ↗ filename` text is a single
/// href-carrying, link-styled `SpanRun` (ADR 0020) — the entire row activates
/// on modifier-click and extracts as one logical line on selection.
fn attachment_run_line(
    attachment: &Attachment,
    index: usize,
    logical_lines: &mut Vec<String>,
) -> RunLine {
    let text = format!("[{}] ↗ {}", index + 1, attachment.filename);
    let logical_line = register_logical_line(logical_lines, text.clone());
    vec![SpanRun {
        text,
        style: theme::link(),
        href: Some(attachment.url.clone()),
        logical_line,
        char_start: 0,
    }]
}

/// The panel's last line (S4): the localized Ctrl/Cmd+click footnote, no
/// `href`. Styled italic+dim via plain `Modifier` flags — carrying no
/// `Color::Rgb`, they need no new `theme.rs` constructor, mirroring
/// `rich_style_to_ratatui`'s own direct use of `Modifier::ITALIC`/`DIM`.
fn attachments_footnote_run_line(logical_lines: &mut Vec<String>) -> RunLine {
    let text = t("Ctrl/Cmd+click opens an attachment");
    let logical_line = register_logical_line(logical_lines, text.clone());
    vec![SpanRun {
        text,
        style: Style::default().add_modifier(Modifier::ITALIC | Modifier::DIM),
        href: None,
        logical_line,
        char_start: 0,
    }]
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
        // A fragment after a wrap seam continues the SAME logical line at the
        // accumulated char offset (ADR 0019 §1) — `run_char_offset` tracks how
        // many of `run`'s own chars have already been placed into prior chunks.
        let mut run_char_offset = 0usize;
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
            let chunk_chars = chunk.chars().count();
            current.push(SpanRun {
                text: chunk.to_owned(),
                style: run.style,
                href: run.href.clone(),
                logical_line: run.logical_line,
                char_start: run.char_start + run_char_offset,
            });
            run_char_offset += chunk_chars;
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
/// `focused_link`. Each rich line registers as one logical line (ADR 0019
/// §1).
fn description_lines_to_runs(
    description: &[RichLine],
    focused_link: Option<usize>,
    logical_lines: &mut Vec<String>,
) -> Vec<RunLine> {
    let mut link_occurrence = 0usize;
    description
        .iter()
        .map(|line| {
            rich_line_to_runs(
                line,
                focused_link,
                &mut link_occurrence,
                logical_lines,
                true,
            )
        })
        .collect()
}

/// Lifts one rich line into a [`RunLine`], registering its full plain text as
/// one logical line (ADR 0019 §1) and accumulating each span's char offset
/// within it. `capture_href` gates whether a span's link mark becomes a
/// clickable `href` — `false` for comment bodies (out of BDR 0010's scope),
/// `true` for the Description panel.
fn rich_line_to_runs(
    line: &RichLine,
    focused_link: Option<usize>,
    link_occurrence: &mut usize,
    logical_lines: &mut Vec<String>,
    capture_href: bool,
) -> RunLine {
    let text: String = line.iter().map(|span| span.text.as_str()).collect();
    let logical_line = register_logical_line(logical_lines, text);
    let mut char_start = 0usize;
    line.iter()
        .map(|span| {
            let style = span_style(span, focused_link, link_occurrence);
            let href = capture_href.then(|| span.style.link.clone()).flatten();
            let char_len = span.text.chars().count();
            let run = SpanRun {
                text: span.text.clone(),
                style,
                href,
                logical_line,
                char_start,
            };
            char_start += char_len;
            run
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

/// The composed detail metadata plus the exact viewport geometry every
/// pointer resolver needs (ADR 0018 §5, ADR 0019 §2): computed once so
/// `detail_link_at`, `detail_pos_at`, and `detail_pos_at_clamped` can never
/// drift from what `render_detail_panels` draws, or from each other. `None`
/// when there is no loaded issue.
struct DetailGeometry {
    compose: DetailCompose,
    content_top: u16,
    content_left: u16,
    viewport_height: u16,
    offset: u16,
}

fn detail_geometry(model: &Model, area: Rect) -> Option<DetailGeometry> {
    let issue = model.detail.as_ref()?;
    let has_status_row = model.status.is_some();
    let content_area = detail_layout_chunks(area, has_status_row)[1];

    let inner_width = content_area.width.saturating_sub(DETAIL_FRAME_BORDER_COLS);
    let compose = compose_detail(
        issue,
        model.detail_focused_link,
        model.detail_focused_comment,
        inner_width,
    );

    let viewport_height = content_area.height.saturating_sub(DETAIL_FRAME_BORDER_ROWS);
    let total_lines = compose.lines.len() as u16;
    let offset = clamp_scroll_offset(model.detail_scroll, total_lines, viewport_height);

    Some(DetailGeometry {
        compose,
        content_top: content_area.y + 1,
        content_left: content_area.x + 1,
        viewport_height,
        offset,
    })
}

/// Resolves an absolute terminal `(x, y)` within the Detail viewport to its
/// composed row index and column, exactly as drawn (ADR 0018 §5, ADR 0019
/// §2). `None` outside the content viewport or with no loaded issue.
fn detail_row_col(geo: &DetailGeometry, x: u16, y: u16) -> Option<(usize, u16)> {
    if y < geo.content_top || x < geo.content_left {
        return None;
    }
    let row_in_viewport = y - geo.content_top;
    if row_in_viewport >= geo.viewport_height {
        return None;
    }
    let row_index = (geo.offset + row_in_viewport) as usize;
    Some((row_index, x - geo.content_left))
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
    let geo = detail_geometry(model, area)?;
    let (row_index, col) = detail_row_col(&geo, x, y)?;
    let row = geo.compose.link_rows.get(row_index)?;
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

/// Maps a viewport cell to its logical `(line, char)` position (ADR 0019
/// §2): the same row/col resolution `detail_link_at` uses, then a
/// display-width walk — first across the row's cells to find the one under
/// `col`, then within that cell's fragment across its chars — so a display
/// column is never mistaken for a char index (BDR 0011 S6). `None` off the
/// content viewport or on a chrome cell/row (no [`CellSpan`]).
pub(super) fn detail_pos_at(model: &Model, area: Rect, x: u16, y: u16) -> Option<(usize, usize)> {
    let geo = detail_geometry(model, area)?;
    let (row_index, col) = detail_row_col(&geo, x, y)?;
    let row = geo.compose.link_rows.get(row_index)?;
    cell_span_at_column(row, &geo.compose.logical_lines, col)
}

/// Walks a composed row's cells by display width (mirrors `cell_at_column`),
/// then within the hit cell walks its chars by display width to the exact
/// logical position (ADR 0019 §2, BDR 0011 S6). `None` past the row's
/// content or on a chrome cell (no [`CellSpan`]).
fn cell_span_at_column(
    row: &[LinkCell],
    logical_lines: &[String],
    col: u16,
) -> Option<(usize, usize)> {
    let mut used = 0u16;
    for cell in row {
        if col < used + cell.width {
            let span = cell.span?;
            let within_col = col - used;
            let ch = char_offset_within_span(logical_lines, span, within_col);
            return Some((span.logical_line, span.char_start + ch));
        }
        used += cell.width;
    }
    None
}

/// The fragment text `span` denotes in `logical_lines` (ADR 0019 §1): a
/// contiguous char slice (never bytes) — the same text a rendered cell
/// carries, reconstructed from the pre-wrap logical line so it's the single
/// source both the highlight and every pointer resolver's column walk read.
fn span_fragment(logical_lines: &[String], span: CellSpan) -> String {
    logical_lines[span.logical_line]
        .chars()
        .skip(span.char_start)
        .take(span.char_len)
        .collect()
}

/// Walks `span`'s fragment by unicode display width to the char index at
/// display column `within_col` (BDR 0011 S6): a display column is never
/// treated as a char index.
fn char_offset_within_span(logical_lines: &[String], span: CellSpan, within_col: u16) -> usize {
    let fragment = span_fragment(logical_lines, span);
    let (prefix, _) = panel::split_at_width(&fragment, within_col);
    prefix.chars().count()
}

/// The inverse walk: the display column at which `char_count` of `span`'s
/// chars have been consumed — lets the highlight convert a char range back
/// to columns without a second geometry pass.
fn column_offset_within_span(logical_lines: &[String], span: CellSpan, char_count: usize) -> u16 {
    let fragment = span_fragment(logical_lines, span);
    let prefix: String = fragment.chars().take(char_count).collect();
    UnicodeWidthStr::width(prefix.as_str()) as u16
}

/// Maps a drag coordinate to a logical `(line, char)` position, clamped (ADR
/// 0019 §2, BDR 0011 S10): a column past a line's content clamps to that
/// line's char count; a row above/below the content clamps to the first/last
/// metadata-bearing visual row. `None` only when the detail has no content.
pub(super) fn detail_pos_at_clamped(
    model: &Model,
    area: Rect,
    x: u16,
    y: u16,
) -> Option<(usize, usize)> {
    let geo = detail_geometry(model, area)?;
    let row_index = clamp_row_index(&geo, y)?;
    let row = geo.compose.link_rows.get(row_index)?;
    let col = x.saturating_sub(geo.content_left);
    Some(pos_in_row_clamped(row, &geo.compose.logical_lines, col))
}

/// Clamps the raw row index for `y` into the range of rows that actually
/// carry selectable content (BDR 0011 S10); `None` when the detail has no
/// content-bearing row at all.
fn clamp_row_index(geo: &DetailGeometry, y: u16) -> Option<usize> {
    let (first, last) = content_row_bounds(&geo.compose.link_rows)?;
    let row_in_viewport = y
        .saturating_sub(geo.content_top)
        .min(geo.viewport_height.saturating_sub(1));
    let raw = (geo.offset + row_in_viewport) as usize;
    Some(raw.clamp(first, last))
}

/// The first and last row indices in `link_rows` carrying at least one
/// content cell (a [`CellSpan`]); `None` when the detail has no content.
fn content_row_bounds(link_rows: &[Vec<LinkCell>]) -> Option<(usize, usize)> {
    let mut indices = link_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|cell| cell.span.is_some()))
        .map(|(i, _)| i);
    let first = indices.next()?;
    let last = indices.next_back().unwrap_or(first);
    Some((first, last))
}

/// Resolves a column within a content-bearing `row`, clamping an off-cell
/// column to the nearest valid logical position (BDR 0011 S10): before the
/// row's first content cell clamps to that line's start; past the last
/// clamps to that line's end (its full char count).
fn pos_in_row_clamped(row: &[LinkCell], logical_lines: &[String], col: u16) -> (usize, usize) {
    if let Some(pos) = cell_span_at_column(row, logical_lines, col) {
        return pos;
    }
    let mut used = 0u16;
    let mut first: Option<(u16, CellSpan)> = None;
    let mut last: Option<CellSpan> = None;
    for cell in row {
        if let Some(span) = cell.span {
            first.get_or_insert((used, span));
            last = Some(span);
        }
        used += cell.width;
    }
    match (first, last) {
        (Some((first_col, first_span)), Some(_)) if col < first_col => {
            (first_span.logical_line, first_span.char_start)
        }
        (_, Some(last_span)) => {
            let line_len = logical_lines[last_span.logical_line].chars().count();
            (last_span.logical_line, line_len)
        }
        _ => (0, 0),
    }
}

/// Normalizes an anchor/cursor pair to reading order (line-major, then char)
/// so a backward or upward drag selects the identical span as a forward one
/// (BDR 0011 S1). Tuple ordering is exactly line-major-then-char.
fn normalize_selection(
    anchor: (usize, usize),
    cursor: (usize, usize),
) -> ((usize, usize), (usize, usize)) {
    if anchor <= cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    }
}

/// The width used to recompose `compose_detail` purely for text extraction
/// (`selection_text`): logical line TEXT is registered before wrapping and is
/// width-independent, so any sufficiently large width reproduces the exact
/// same `logical_lines` the on-screen compose used (the one narrow exception
/// — a Details meta value long enough to ellipsize at the real viewport width
/// — is not exercised by any selectable content in practice).
const DETAIL_TEXT_EXTRACTION_WIDTH: u16 = u16::MAX;

/// Extracts the selected span's text (ADR 0019 §1): normalizes `(anchor,
/// cursor)` to reading order, slices `compose_detail`'s chrome-free
/// `logical_lines` by char index (UTF-8-safe `chars().skip/take`, never
/// bytes), and joins multi-line spans with `\n`. `None` with no selection, no
/// loaded issue, or an empty span.
pub(super) fn selection_text(model: &Model) -> Option<String> {
    let issue = model.detail.as_ref()?;
    let selection = model.selection.as_ref()?;
    let (start, end) = normalize_selection(selection.anchor, selection.cursor);
    let compose = compose_detail(
        issue,
        model.detail_focused_link,
        model.detail_focused_comment,
        DETAIL_TEXT_EXTRACTION_WIDTH,
    );
    let text = extract_selection_text(&compose.logical_lines, start, end);
    (!text.is_empty()).then_some(text)
}

/// Slices `line`'s chars in `[start, end)` (UTF-8-safe, clamped to the
/// line's own length); `""` when `line` is absent (out-of-range index).
fn slice_line_chars(line: Option<&String>, start: usize, end: usize) -> String {
    line.map(|text| {
        text.chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect()
    })
    .unwrap_or_default()
}

/// Joins the selected span's text across `logical_lines`, `\n`-separated
/// (BDR 0011 S7: a selection across a wrap seam yields the contiguous
/// logical text — since both fragments read the SAME logical line, nothing
/// is dropped or repeated).
fn extract_selection_text(
    logical_lines: &[String],
    start: (usize, usize),
    end: (usize, usize),
) -> String {
    let (start_line, start_char) = start;
    let (end_line, end_char) = end;
    if start_line == end_line {
        return slice_line_chars(logical_lines.get(start_line), start_char, end_char);
    }
    let mut parts = vec![slice_line_chars(
        logical_lines.get(start_line),
        start_char,
        usize::MAX,
    )];
    for line_idx in (start_line + 1)..end_line {
        parts.push(logical_lines.get(line_idx).cloned().unwrap_or_default());
    }
    parts.push(slice_line_chars(logical_lines.get(end_line), 0, end_char));
    parts.join("\n")
}

/// Patches `theme::selection_highlight()` onto the spans covered by an
/// active `selection` (ADR 0019 §5), reading the SAME cell metadata
/// `detail_pos_at` walks — no second geometry computation. Returns
/// `compose.lines` unchanged with no active selection.
fn apply_selection_highlight(
    compose: DetailCompose,
    selection: Option<&Selection>,
) -> Vec<Line<'static>> {
    let DetailCompose {
        lines,
        link_rows,
        logical_lines,
    } = compose;
    let Some(selection) = selection else {
        return lines;
    };
    let (start, end) = normalize_selection(selection.anchor, selection.cursor);
    lines
        .into_iter()
        .zip(link_rows.iter())
        .map(
            |(line, row)| match selection_highlight_columns(row, &logical_lines, start, end) {
                Some((from, to)) => apply_span_highlight(&line, from, to),
                None => line,
            },
        )
        .collect()
}

/// The display-column range within `row` covered by the normalized selection
/// `[start, end)` (ADR 0019 §5): reads the same cell metadata `detail_pos_at`
/// walks. `None` when none of the row's cells fall inside the selection.
fn selection_highlight_columns(
    row: &[LinkCell],
    logical_lines: &[String],
    start: (usize, usize),
    end: (usize, usize),
) -> Option<(u16, u16)> {
    let mut used = 0u16;
    let mut covered: Option<(u16, u16)> = None;

    for cell in row {
        if let Some(span) = cell.span {
            if let Some((from, to)) = cell_selection_overlap(span, start, end) {
                let col_from =
                    used + column_offset_within_span(logical_lines, span, from - span.char_start);
                let col_to =
                    used + column_offset_within_span(logical_lines, span, to - span.char_start);
                covered = Some(match covered {
                    Some((c_from, c_to)) => (c_from.min(col_from), c_to.max(col_to)),
                    None => (col_from, col_to),
                });
            }
        }
        used += cell.width;
    }
    covered
}

/// The char sub-range of `span` selected by the normalized `[start, end)`
/// span (BDR 0011 S1/S7): `None` when `span`'s logical line falls outside
/// `[start_line, end_line]` or the intersection is empty.
fn cell_selection_overlap(
    span: CellSpan,
    (start_line, start_char): (usize, usize),
    (end_line, end_char): (usize, usize),
) -> Option<(usize, usize)> {
    if span.logical_line < start_line || span.logical_line > end_line {
        return None;
    }
    let cell_end = span.char_start + span.char_len;
    let lower = if span.logical_line == start_line {
        start_char
    } else {
        0
    };
    let upper = if span.logical_line == end_line {
        end_char
    } else {
        cell_end
    };
    let from = span.char_start.max(lower);
    let to = cell_end.min(upper);
    (from < to).then_some((from, to))
}

/// Splits `line`'s spans at the exact display-column boundaries `[start_col,
/// end_col)` and patches `theme::selection_highlight()` onto the covered
/// portion (ADR 0019 §5, BDR 0011 S1): partial cell coverage splits a span at
/// the exact char boundary using the same width walk `detail_pos_at` uses
/// (via `panel::split_at_width`), so highlight and extraction never drift.
fn apply_span_highlight(line: &Line<'static>, start_col: u16, end_col: u16) -> Line<'static> {
    let highlight = theme::selection_highlight();
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    let mut used = 0u16;

    for span in &line.spans {
        let text = span.content.as_ref();
        let width = UnicodeWidthStr::width(text) as u16;
        let span_start = used;
        let span_end = used + width;
        used = span_end;

        if span_end <= start_col || span_start >= end_col {
            spans.push(span.clone());
            continue;
        }

        let before_cols = start_col.saturating_sub(span_start);
        let highlighted_end_cols = end_col.saturating_sub(span_start);

        let (before, rest) = panel::split_at_width(text, before_cols);
        let (highlighted, after) = panel::split_at_width(rest, highlighted_end_cols - before_cols);

        if !before.is_empty() {
            spans.push(Span::styled(before.to_owned(), span.style));
        }
        if !highlighted.is_empty() {
            spans.push(Span::styled(
                highlighted.to_owned(),
                span.style.patch(highlight),
            ));
        }
        if !after.is_empty() {
            spans.push(Span::styled(after.to_owned(), span.style));
        }
    }
    Line::from(spans)
}
