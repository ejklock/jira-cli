//! The reusable centered modal overlay primitive (ADR 0024 §2, BDR 0015 S5):
//! `modal_area` is a pure layout function (centers, clamps with a margin,
//! never overflows) and `render_modal` draws through it — dimming the
//! backdrop, `Clear`ing the box, then drawing a rounded bordered panel with
//! title/body/hint/status and optional buttons. `button_targets` exposes the
//! same button geometry `render_buttons` draws in ABSOLUTE frame coordinates
//! (ADR 0024 §2d) so a caller can hit-test a click without rendering; the
//! delete-confirm modal's Sim/Não click (C4e) is its first consumer.
#![allow(dead_code)]

use ratatui::{
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::theme;

/// The gap (columns/rows) the modal box always keeps clear of the frame's
/// own edge, even when the desired size would otherwise touch it.
const MODAL_MARGIN: u16 = 1;

/// The modal box's target size as a percentage of the frame (ADR 0024 §2b),
/// before the content-driven minimum and the `modal_area` clamp apply.
const MODAL_WIDTH_PERCENT: u32 = 70;
const MODAL_HEIGHT_PERCENT: u32 = 70;

/// The smallest box `render_modal` will target regardless of content, so a
/// title-only modal never collapses to an unreadable sliver.
const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 5;

/// One button rendered inside a modal (ADR 0024 §2d): `id` is the opaque
/// token a caller matches against a click; `label` is the display text.
pub struct ModalButton {
    pub id: String,
    pub label: String,
}

/// The content a modal renders (ADR 0024 §2). `body` reuses the TUI's rich
/// run channel — the same `Vec<Line<'static>>` `panel::panel_box` composes —
/// so callers hand `render_modal` already-styled, already-wrapped text.
/// `buttons` is empty for the compose adapter (C3b); C4's confirm dialog is
/// the first caller to populate it.
pub struct ModalContent {
    pub title: String,
    pub body: Vec<Line<'static>>,
    pub hint: Option<String>,
    pub status: Option<String>,
    pub buttons: Vec<ModalButton>,
}

/// One button's click target, in coordinates relative to the `ModalRender`'s
/// own `area` (ADR 0024 §2d) — a caller adds the modal's `x`/`y` to recover
/// absolute frame coordinates for hit-testing a mouse event.
pub struct ButtonTarget {
    pub id: String,
    pub area: Rect,
}

/// `render_modal`'s result (ADR 0024 §2d): the modal's own `Rect` plus any
/// button click targets, so a caller can register clicks without
/// `render_modal` knowing anything about input handling.
pub struct ModalRender {
    pub area: Rect,
    pub buttons: Vec<ButtonTarget>,
}

/// Centers a `desired_w` x `desired_h` box within `frame_area`, clamped so it
/// never overflows `frame_area` even on a small/narrow terminal (a
/// `MODAL_MARGIN`-column/row gap is always kept clear). Pure — no `Frame`,
/// no I/O — so it is headlessly unit-tested (ADR 0024 §2, BDR 0015 S5).
pub fn modal_area(frame_area: Rect, desired_w: u16, desired_h: u16) -> Rect {
    let width = clamped_dimension(desired_w, frame_area.width);
    let height = clamped_dimension(desired_h, frame_area.height);
    Rect {
        x: frame_area.x + centered_offset(frame_area.width, width),
        y: frame_area.y + centered_offset(frame_area.height, height),
        width,
        height,
    }
}

fn clamped_dimension(desired: u16, available: u16) -> u16 {
    let margin = (MODAL_MARGIN * 2).min(available);
    desired.min(available.saturating_sub(margin))
}

fn centered_offset(available: u16, used: u16) -> u16 {
    available.saturating_sub(used) / 2
}

/// Draws `content` as a centered modal over `frame_area`: (a) strongly dims
/// every backdrop cell in `frame_area` (`Modifier::DIM` plus a dark
/// background, patched onto the already-rendered frame); (b) sizes the box
/// to `MODAL_*_PERCENT` of the frame with a content-driven minimum, centered
/// via `modal_area`; (c) `Clear`s the box; (d) draws the rounded bordered
/// panel with title/body/hint/status and registers any buttons. Returns the
/// modal `Rect` plus button click targets in modal-relative coordinates.
pub fn render_modal(frame: &mut Frame, frame_area: Rect, content: &ModalContent) -> ModalRender {
    dim_backdrop(frame, frame_area);

    let (desired_w, desired_h) = desired_size(frame_area, content);
    let area = modal_area(frame_area, desired_w, desired_h);
    frame.render_widget(Clear, area);

    let block = modal_block(&content.title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = split_rows(inner, content);
    render_body(frame, rows.body, content);
    render_optional_line(frame, rows.status, content.status.as_deref());
    render_optional_line(frame, rows.hint, content.hint.as_deref());
    let buttons = render_buttons(frame, rows.buttons, &content.buttons)
        .into_iter()
        .map(|target| ButtonTarget {
            id: target.id,
            area: relative_to(target.area, area),
        })
        .collect();

    ModalRender { area, buttons }
}

/// The button click targets `render_modal` would register for `content`,
/// computed WITHOUT rendering, in ABSOLUTE `frame_area` coordinates (ADR 0024
/// §2d): `desired_size` -> `modal_area` -> the modal block's inner rect ->
/// `split_rows`' buttons row -> [`layout_button_targets`] — the exact chain
/// `render_modal` walks before it maps targets modal-relative for its own
/// `ModalRender`, so a caller's hit-test can never drift from what's drawn.
pub fn button_targets(frame_area: Rect, content: &ModalContent) -> Vec<ButtonTarget> {
    let (desired_w, desired_h) = desired_size(frame_area, content);
    let area = modal_area(frame_area, desired_w, desired_h);
    let inner = modal_block(&content.title).inner(area);
    let buttons_row = split_rows(inner, content).buttons;
    layout_button_targets(buttons_row, &content.buttons)
}

fn dim_backdrop(frame: &mut Frame, frame_area: Rect) {
    frame
        .buffer_mut()
        .set_style(frame_area, theme::modal_backdrop());
}

fn modal_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::modal_border())
        .style(theme::modal_background())
        .title(title.to_owned())
}

/// The box size `render_modal` targets before the `modal_area` clamp:
/// `MODAL_*_PERCENT` of `frame_area`, floored at a content-driven minimum so
/// a short body/hint never renders inside a box too small to hold it.
fn desired_size(frame_area: Rect, content: &ModalContent) -> (u16, u16) {
    let width = percent_of(frame_area.width, MODAL_WIDTH_PERCENT).max(min_content_width(content));
    let height =
        percent_of(frame_area.height, MODAL_HEIGHT_PERCENT).max(min_content_height(content));
    (width, height)
}

fn percent_of(value: u16, percent: u32) -> u16 {
    ((value as u32 * percent) / 100) as u16
}

const BORDER_COLS: u16 = 2;
const BORDER_ROWS: u16 = 2;

fn min_content_width(content: &ModalContent) -> u16 {
    let mut longest = display_width(&content.title);
    longest = longest.max(content.hint.as_deref().map_or(0, display_width));
    longest = longest.max(content.status.as_deref().map_or(0, display_width));
    for line in &content.body {
        longest = longest.max(line_width(line));
    }
    (longest as u16).saturating_add(BORDER_COLS).max(MIN_WIDTH)
}

fn min_content_height(content: &ModalContent) -> u16 {
    let body_rows = content.body.len() as u16;
    BORDER_ROWS
        .saturating_add(body_rows)
        .saturating_add(reserved_rows(content))
        .max(MIN_HEIGHT)
}

fn reserved_rows(content: &ModalContent) -> u16 {
    u16::from(content.status.is_some())
        + u16::from(!content.buttons.is_empty())
        + u16::from(content.hint.is_some())
}

fn line_width(line: &Line<'static>) -> usize {
    line.spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum()
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

struct ModalRows {
    body: Rect,
    status: Rect,
    buttons: Rect,
    hint: Rect,
}

/// Stacks the inner content area into body/status/buttons/hint rows (ADR
/// 0024 §2c): the body takes whatever height remains after reserving exactly
/// one row per present optional element, with the hint — the modal's
/// persistent chrome — always the bottommost row.
fn split_rows(inner: Rect, content: &ModalContent) -> ModalRows {
    let body_height = inner.height.saturating_sub(reserved_rows(content));
    let body = Rect {
        height: body_height,
        ..inner
    };
    let status = row_after(body, u16::from(content.status.is_some()));
    let buttons = row_after(status, u16::from(!content.buttons.is_empty()));
    let hint = row_after(buttons, u16::from(content.hint.is_some()));
    ModalRows {
        body,
        status,
        buttons,
        hint,
    }
}

fn row_after(prev: Rect, height: u16) -> Rect {
    Rect {
        x: prev.x,
        y: prev.y + prev.height,
        width: prev.width,
        height,
    }
}

fn render_body(frame: &mut Frame, area: Rect, content: &ModalContent) {
    let paragraph =
        Paragraph::new(Text::from(content.body.clone())).style(theme::modal_background());
    frame.render_widget(paragraph, area);
}

fn render_optional_line(frame: &mut Frame, area: Rect, text: Option<&str>) {
    let Some(text) = text else { return };
    let paragraph = Paragraph::new(text.to_owned()).style(theme::modal_hint());
    frame.render_widget(paragraph, area);
}

/// The per-button x-advance layout: each button's `[ label ]` rect, left to
/// right starting at `buttons_row.x`, advancing by the label's display width
/// plus a 2-column gap. The SINGLE geometry source [`render_buttons`] and
/// [`button_targets`] both build on, so the rendered button cells and a
/// click hit-test can never drift apart.
fn layout_button_targets(buttons_row: Rect, buttons: &[ModalButton]) -> Vec<ButtonTarget> {
    let mut targets = Vec::with_capacity(buttons.len());
    let mut x = buttons_row.x;
    for button in buttons {
        let width = display_width(&button_label(button)) as u16;
        targets.push(ButtonTarget {
            id: button.id.clone(),
            area: Rect {
                x,
                y: buttons_row.y,
                width,
                height: 1,
            },
        });
        x += width + 2;
    }
    targets
}

fn button_label(button: &ModalButton) -> String {
    format!("[ {} ]", button.label)
}

fn render_buttons(frame: &mut Frame, area: Rect, buttons: &[ModalButton]) -> Vec<ButtonTarget> {
    let targets = layout_button_targets(area, buttons);
    if targets.is_empty() {
        return targets;
    }

    let mut spans = Vec::with_capacity(buttons.len() * 2);
    for button in buttons {
        spans.push(Span::styled(button_label(button), theme::modal_border()));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    targets
}

fn relative_to(rect: Rect, origin: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_sub(origin.x),
        y: rect.y.saturating_sub(origin.y),
        width: rect.width,
        height: rect.height,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tui/modal.rs"]
mod tests;
