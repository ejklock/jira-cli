//! Truecolor theme for the browse TUI (ADR 0014 §1-§2): a fixed sober
//! cool-retro palette exposed as `ratatui::style::Style` builders. Every
//! `Color::Rgb` literal in the TUI lives here — callers reach for these
//! functions instead of hardcoding colors, so the palette stays a single
//! source of truth.

use ratatui::style::{Color, Modifier, Style};

/// The identity header bar shown above the content on every browse screen.
pub fn header_bar() -> Style {
    Style::default()
        .fg(Color::Rgb(102, 204, 204))
        .bg(Color::Rgb(38, 52, 74))
        .add_modifier(Modifier::BOLD)
}

// Issue 0030 lands the full palette up front (ADR 0014 §1); `badge`,
// `selected`, `due_overdue` and `due_near` gained production callers in the
// D2 list-card renderer (issue 0031); `section_title` gained one in the D3
// detail panels (issue 0032). `column_header`/`link` still await later
// H2/H3 slices, so a plain `cargo build` (which excludes `#[cfg(test)]`)
// would otherwise see them as unused.

/// A section title inside a screen's content region.
pub fn section_title() -> Style {
    Style::default()
        .fg(Color::Rgb(102, 204, 204))
        .add_modifier(Modifier::BOLD)
}

/// A list/table column header.
#[allow(dead_code)]
pub fn column_header() -> Style {
    Style::default()
        .fg(Color::Rgb(140, 165, 196))
        .add_modifier(Modifier::BOLD)
}

/// The currently selected row or item.
pub fn selected() -> Style {
    Style::default()
        .fg(Color::Rgb(13, 13, 13))
        .bg(Color::Rgb(210, 160, 90))
        .add_modifier(Modifier::BOLD)
}

/// A status/priority badge.
pub fn badge() -> Style {
    Style::default()
        .fg(Color::Rgb(210, 160, 90))
        .add_modifier(Modifier::BOLD)
}

/// An inline hyperlink run.
#[allow(dead_code)]
pub fn link() -> Style {
    Style::default()
        .fg(Color::Rgb(120, 190, 130))
        .add_modifier(Modifier::UNDERLINED)
}

/// The footer hint bar shown below the content on every browse screen.
pub fn footer() -> Style {
    Style::default()
        .fg(Color::Rgb(208, 216, 224))
        .bg(Color::Rgb(38, 52, 74))
        .add_modifier(Modifier::BOLD)
}

/// An overdue due date.
pub fn due_overdue() -> Style {
    Style::default().fg(Color::Rgb(224, 108, 108))
}

/// A due date approaching soon.
pub fn due_near() -> Style {
    Style::default().fg(Color::Rgb(210, 160, 90))
}

/// The thin status row's error variant (BDR 0007 S8): the overdue red on the
/// footer bar's steel-blue background.
pub fn status_error() -> Style {
    Style::default()
        .fg(Color::Rgb(224, 108, 108))
        .bg(Color::Rgb(38, 52, 74))
        .add_modifier(Modifier::BOLD)
}
