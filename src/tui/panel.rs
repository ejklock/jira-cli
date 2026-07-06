//! Pure panel geometry for the browse TUI detail (ADR 0014 §4, BDR 0007
//! S5-S6): a rounded box with a label embedded in the top border, plus the
//! display-width helpers (`fit_to_display_width`, `ellipsize_display`) that
//! keep every emitted line exactly the requested number of terminal columns
//! even with CJK/wide or zero-width glyphs. No I/O, no terminal — pure
//! `Line`/`Span` building, so it is unit-testable without a `Frame`.

use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::theme;

const CORNER_TOP_LEFT: char = '╭';
const CORNER_TOP_RIGHT: char = '╮';
const CORNER_BOTTOM_LEFT: char = '╰';
const CORNER_BOTTOM_RIGHT: char = '╯';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';

/// The smallest box that still has room for both corners and a closing
/// border on every line.
const MIN_WIDTH: u16 = 4;

/// The interior content width available inside a `panel_box` of `width`
/// display columns — a border char and one padding column on each side.
/// Exposed so callers that pre-wrap body lines (description/comment text)
/// compute the same budget `panel_box` fits them to.
pub fn inner_content_width(width: u16) -> u16 {
    width.max(MIN_WIDTH) - 4
}

/// Draws a rounded box around `body`: `label` is embedded in the top border
/// (styled `theme::section_title()`, ellipsized when it would overflow), one
/// column of interior horizontal padding surrounds the content, and every
/// emitted line — including the borders — is fit to exactly `width` display
/// columns so the right border always closes, even with wide glyphs.
pub fn panel_box(label: &str, body: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let width = width.max(MIN_WIDTH);
    let inner_width = inner_content_width(width);
    let mut lines = Vec::with_capacity(body.len() + 2);
    lines.push(top_border(label, width));
    lines.extend(body.iter().map(|line| content_line(line, inner_width)));
    lines.push(bottom_border(width));
    lines
}

fn top_border(label: &str, width: u16) -> Line<'static> {
    let budget = (width - 2) as usize;
    let label = ellipsize_display(label, budget.saturating_sub(3) as u16);
    let mut spans = vec![
        Span::raw(CORNER_TOP_LEFT.to_string()),
        Span::raw(HORIZONTAL.to_string()),
    ];
    if label.is_empty() {
        spans.push(Span::raw(
            HORIZONTAL.to_string().repeat(budget.saturating_sub(1)),
        ));
    } else {
        let used = display_width(&label) + 3;
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label, theme::section_title()));
        spans.push(Span::raw(" "));
        spans.push(Span::raw(
            HORIZONTAL.to_string().repeat(budget.saturating_sub(used)),
        ));
    }
    spans.push(Span::raw(CORNER_TOP_RIGHT.to_string()));
    fit_to_display_width(&Line::from(spans), width)
}

fn bottom_border(width: u16) -> Line<'static> {
    let fill = (width - 2) as usize;
    Line::from(format!(
        "{CORNER_BOTTOM_LEFT}{}{CORNER_BOTTOM_RIGHT}",
        HORIZONTAL.to_string().repeat(fill)
    ))
}

fn content_line(line: &Line<'static>, inner_width: u16) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{VERTICAL} "))];
    spans.extend(fit_to_display_width(line, inner_width).spans);
    spans.push(Span::raw(format!(" {VERTICAL}")));
    Line::from(spans)
}

/// Pads or truncates `line` to exactly `cols` display columns. Styled spans
/// are preserved and the last kept span is truncated positionally as needed;
/// any shortfall (a wide glyph that would overflow, or a shorter line) is
/// covered by an unstyled trailing pad so the total is always exact.
pub fn fit_to_display_width(line: &Line<'static>, cols: u16) -> Line<'static> {
    let cols = cols as usize;
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;

    for span in &line.spans {
        if used >= cols {
            break;
        }
        let (kept, _) = split_at_width(span.content.as_ref(), (cols - used) as u16);
        if kept.is_empty() {
            continue;
        }
        used += display_width(kept);
        spans.push(Span::styled(kept.to_owned(), span.style));
    }

    if used < cols {
        spans.push(Span::raw(" ".repeat(cols - used)));
    }
    Line::from(spans)
}

/// Truncates `text` to fit within `cols` display columns, appending a
/// single-column `…` when truncation occurs. Returns `text` unchanged when
/// it already fits, and an empty string when `cols` is 0.
pub fn ellipsize_display(text: &str, cols: u16) -> String {
    if cols == 0 {
        return String::new();
    }
    if display_width(text) <= cols as usize {
        return text.to_owned();
    }
    let (kept, _) = split_at_width(text, cols - 1);
    format!("{kept}…")
}

/// Splits `text` at the largest prefix whose display width is `<= cols`,
/// returning `(prefix, rest)` at a char boundary. A wide glyph that would
/// overflow the budget is left in `rest` — the caller pads the leftover
/// column rather than splitting the glyph.
pub(crate) fn split_at_width(text: &str, cols: u16) -> (&str, &str) {
    let cols = cols as usize;
    if cols == 0 {
        return ("", text);
    }
    let mut used = 0usize;
    for (idx, ch) in text.char_indices() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > cols {
            return (&text[..idx], &text[idx..]);
        }
        used += w;
    }
    (text, "")
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}
