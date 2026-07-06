use super::model::{header_line, Identity};
use super::theme;
use super::*;

use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::IssueRow;
use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier, Style},
    Terminal,
};

// ---- Helpers ----

fn make_row(key: &str) -> IssueRow {
    IssueRow {
        key: key.to_owned(),
        issue_type: "Task".to_owned(),
        status: "Open".to_owned(),
        assignee: Some("Alice".to_owned()),
        summary: "Fix something".to_owned(),
    }
}

fn make_list_model(identities: Vec<Identity>) -> Model {
    Model {
        rows: vec![make_row("PROJ-1")],
        selected: 0,
        screen: Screen::List,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: "https://test.atlassian.net".to_owned(),
        jql: "assignee = currentUser()".to_owned(),
        next_page_token: None,
        detail_links: vec![],
        detail_focused_link: None,
        identities,
    }
}

fn make_detail_model(identities: Vec<Identity>) -> Model {
    let mut model = make_list_model(identities);
    model.screen = Screen::Detail;
    model.detail = Some(crate::test_support::issue("PROJ-1"));
    model
}

fn render_to_buffer(model: &Model, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(model, frame)).unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buf: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buf.area.width)
        .map(|col| buf[(col, row)].symbol().to_owned())
        .collect()
}

fn cell_style(buf: &ratatui::buffer::Buffer, col: u16, row: u16) -> Style {
    buf[(col, row)].style()
}

/// Asserts a rendered cell carries the given bar style's fg/bg/BOLD — compares
/// components rather than full `Style` equality because `Buffer::set_style`
/// patches onto the cell's existing (already-initialized) style, so a
/// rendered cell's `underline_color` differs from a freshly built `Style`
/// even when fg/bg/modifiers match.
fn assert_bar_style(style: Style, fg: Color, bg: Color) {
    assert_eq!(style.fg, Some(fg), "unexpected fg: {style:?}");
    assert_eq!(style.bg, Some(bg), "unexpected bg: {style:?}");
    assert!(
        style.add_modifier.contains(Modifier::BOLD),
        "bar style must be bold: {style:?}"
    );
}

fn single_identity() -> Vec<Identity> {
    vec![Identity {
        email: "me@x.com".to_owned(),
        instance: "acme".to_owned(),
    }]
}

fn two_identities() -> Vec<Identity> {
    vec![
        Identity {
            email: "me@x.com".to_owned(),
            instance: "acme".to_owned(),
        },
        Identity {
            email: "you@y.com".to_owned(),
            instance: "beta".to_owned(),
        },
    ]
}

// ---- BDR 0007 S1: header_line pure helper ----

#[test]
fn header_line_with_no_identities_is_empty() {
    assert_eq!(header_line(&[]), "");
}

#[test]
fn header_line_with_one_identity_shows_email_and_instance() {
    assert_eq!(header_line(&single_identity()), "me@x.com · acme");
}

#[test]
fn header_line_with_three_identities_shows_first_and_plus_two_more() {
    let identities = vec![
        Identity {
            email: "me@x.com".to_owned(),
            instance: "acme".to_owned(),
        },
        Identity {
            email: "you@y.com".to_owned(),
            instance: "beta".to_owned(),
        },
        Identity {
            email: "them@z.com".to_owned(),
            instance: "gamma".to_owned(),
        },
    ];
    assert_eq!(
        header_line(&identities),
        "me@x.com · acme (+2 more)",
        "the third identity must be reflected in the '+N more' count"
    );
}

// ---- BDR 0007 S1: list/detail render the header on row 0 ----

#[test]
fn view_list_header_renders_single_identity_in_header_bar_style() {
    let model = make_list_model(single_identity());

    let buf = render_to_buffer(&model, 120, 20);

    assert_eq!(row_text(&buf, 0).trim_end(), "me@x.com · acme");
    assert_bar_style(
        cell_style(&buf, 0, 0),
        Color::Rgb(102, 204, 204),
        Color::Rgb(38, 52, 74),
    );
}

#[test]
fn view_list_header_renders_two_identities_with_plus_one_more() {
    let model = make_list_model(two_identities());

    let buf = render_to_buffer(&model, 120, 20);

    assert_eq!(row_text(&buf, 0).trim_end(), "me@x.com · acme (+1 more)");
}

#[test]
fn view_list_header_with_no_identities_renders_empty_row_without_panic() {
    let model = make_list_model(vec![]);

    let buf = render_to_buffer(&model, 120, 20);

    assert_eq!(row_text(&buf, 0).trim_end(), "");
}

#[test]
fn view_detail_header_renders_identity_in_header_bar_style_on_row_zero() {
    let model = make_detail_model(single_identity());

    let buf = render_to_buffer(&model, 120, 30);

    assert_eq!(row_text(&buf, 0).trim_end(), "me@x.com · acme");
    assert_bar_style(
        cell_style(&buf, 0, 0),
        Color::Rgb(102, 204, 204),
        Color::Rgb(38, 52, 74),
    );
}

#[test]
fn view_detail_header_with_no_identities_renders_empty_row_without_panic() {
    let model = make_detail_model(vec![]);

    let buf = render_to_buffer(&model, 120, 30);

    assert_eq!(row_text(&buf, 0).trim_end(), "");
}

// ---- Footer restyled through theme::footer(); text unchanged ----

#[test]
fn view_list_footer_uses_theme_footer_style() {
    let model = make_list_model(vec![]);

    let buf = render_to_buffer(&model, 120, 20);

    assert_bar_style(
        cell_style(&buf, 0, 19),
        Color::Rgb(208, 216, 224),
        Color::Rgb(38, 52, 74),
    );
}

#[test]
fn view_detail_footer_uses_theme_footer_style() {
    let model = make_detail_model(vec![]);

    let buf = render_to_buffer(&model, 120, 30);

    assert_bar_style(
        cell_style(&buf, 0, 29),
        Color::Rgb(208, 216, 224),
        Color::Rgb(38, 52, 74),
    );
}

#[test]
fn view_list_footer_hint_text_is_unchanged() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_list_model(vec![]);
    let buf = render_to_buffer(&model, 120, 20);

    assert!(
        row_text(&buf, 19).contains("↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"),
        "footer hint text must be unchanged; got: {:?}",
        row_text(&buf, 19)
    );

    set_language("en");
}

#[test]
fn view_detail_footer_hint_text_is_unchanged() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_detail_model(vec![]);
    let buf = render_to_buffer(&model, 120, 30);

    assert!(
        row_text(&buf, 29).contains("↑/↓ j/k scroll  Esc/b back  q quit"),
        "footer hint text must be unchanged; got: {:?}",
        row_text(&buf, 29)
    );

    set_language("en");
}

// ---- ADR 0014 §1: remaining theme palette functions (used only here so
// far; later H2/H3 slices apply them to table/badge/link rendering) ----

#[test]
fn theme_section_title_matches_adr_0014_palette() {
    assert_eq!(
        theme::section_title(),
        Style::default()
            .fg(Color::Rgb(102, 204, 204))
            .add_modifier(Modifier::BOLD)
    );
}

#[test]
fn theme_column_header_matches_adr_0014_palette() {
    assert_eq!(
        theme::column_header(),
        Style::default()
            .fg(Color::Rgb(140, 165, 196))
            .add_modifier(Modifier::BOLD)
    );
}

#[test]
fn theme_selected_matches_adr_0014_palette() {
    assert_eq!(
        theme::selected(),
        Style::default()
            .fg(Color::Rgb(13, 13, 13))
            .bg(Color::Rgb(210, 160, 90))
            .add_modifier(Modifier::BOLD)
    );
}

#[test]
fn theme_badge_matches_adr_0014_palette() {
    assert_eq!(
        theme::badge(),
        Style::default()
            .fg(Color::Rgb(210, 160, 90))
            .add_modifier(Modifier::BOLD)
    );
}

#[test]
fn theme_link_matches_adr_0014_palette() {
    assert_eq!(
        theme::link(),
        Style::default()
            .fg(Color::Rgb(120, 190, 130))
            .add_modifier(Modifier::UNDERLINED)
    );
}

#[test]
fn theme_due_overdue_matches_adr_0014_palette() {
    assert_eq!(
        theme::due_overdue(),
        Style::default().fg(Color::Rgb(224, 108, 108))
    );
}

#[test]
fn theme_due_near_matches_adr_0014_palette() {
    assert_eq!(
        theme::due_near(),
        Style::default().fg(Color::Rgb(210, 160, 90))
    );
}
