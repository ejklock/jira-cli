use super::model::{header_line, Identity};
use super::theme;
use super::view;
use super::*;

use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::IssueRow;
use crate::test_support::duedate_offset_from_today;
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
        duedate: None,
        project: None,
    }
}

/// Builds a fully-specified card row (BDR 0007 S2/S3): every field a card
/// can display is explicit, so tests read as the scenario they assert.
fn make_card_row(
    key: &str,
    summary: &str,
    status: &str,
    project: Option<&str>,
    duedate: Option<String>,
) -> IssueRow {
    IssueRow {
        key: key.to_owned(),
        issue_type: "Task".to_owned(),
        status: status.to_owned(),
        assignee: Some("Alice".to_owned()),
        summary: summary.to_owned(),
        duedate,
        project: project.map(str::to_owned),
    }
}

fn make_list_model_with_rows(rows: Vec<IssueRow>, selected: usize) -> Model {
    Model {
        rows,
        selected,
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
        identities: vec![],
    }
}

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    (0..buf.area.height)
        .map(|row| row_text(buf, row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Finds `needle` anywhere in the buffer and returns the style of its first
/// cell — mirrors `tests/unit/tui.rs`'s helper of the same purpose.
fn style_at_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<Style> {
    for row in 0..buf.area.height {
        let text = row_text(buf, row);
        if let Some(start) = text.find(needle) {
            let col = text[..start].chars().count() as u16;
            return Some(cell_style(buf, col, row));
        }
    }
    None
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

// ---- ADR 0014 §1: remaining theme palette functions (section_title,
// column_header, link await later H2/H3 slices) ----

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

// ---- BDR 0007 S2: issue card ----

#[test]
fn view_list_renders_issue_as_bordered_card_with_due_status_and_project() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row(
        "PROJ-1",
        "Fix login",
        "In Progress",
        Some("Proj"),
        Some(duedate_offset_from_today(1)),
    );
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);

    let buf = render_to_buffer(&model, 60, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("PROJ-1 Fix login"),
        "card line 1 must be 'PROJ-1 Fix login'; got: {text}"
    );
    assert!(
        text.contains("tomorrow · In Progress · Proj"),
        "card line 2 must be 'tomorrow · In Progress · Proj'; got: {text}"
    );

    let due_style = style_at_text(&buf, "tomorrow").expect("due segment must appear in buffer");
    assert_eq!(
        due_style.fg,
        Some(Color::Rgb(210, 160, 90)),
        "due segment due-tomorrow must carry the near-amber style: {due_style:?}"
    );

    set_language("en");
}

#[test]
fn view_list_card_omits_absent_project_and_status_segments() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row("PROJ-2", "No project set", "", None, None);
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);

    let buf = render_to_buffer(&model, 60, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("no due date"),
        "due segment must fall back to 'no due date'; got: {text}"
    );
    assert!(
        !text.contains("no due date ·"),
        "an omitted status/project must leave no dangling separator; got: {text}"
    );

    set_language("en");
}

// ---- BDR 0007 S3: due colors ----

#[test]
fn view_list_due_yesterday_renders_overdue_in_red() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row(
        "PROJ-3",
        "Overdue issue",
        "Open",
        None,
        Some(duedate_offset_from_today(-1)),
    );
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(buffer_text(&buf).contains("overdue by 1 day"));
    let style =
        style_at_text(&buf, "overdue by 1 day").expect("overdue segment must appear in buffer");
    assert_eq!(style.fg, Some(Color::Rgb(224, 108, 108)));

    set_language("en");
}

#[test]
fn view_list_due_today_renders_amber() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row(
        "PROJ-4",
        // summary must not contain "today" itself, or style_at_text would match
        // this title line instead of the due segment on line 2.
        "Due date issue",
        "Open",
        None,
        Some(duedate_offset_from_today(0)),
    );
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(buffer_text(&buf).contains("today"));
    let style = style_at_text(&buf, "today").expect("today segment must appear in buffer");
    assert_eq!(style.fg, Some(Color::Rgb(210, 160, 90)));

    set_language("en");
}

#[test]
fn view_list_due_in_five_days_renders_default_style() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row(
        "PROJ-5",
        "Due later issue",
        "Open",
        None,
        Some(duedate_offset_from_today(5)),
    );
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(buffer_text(&buf).contains("in 5 days"));
    let style = style_at_text(&buf, "in 5 days").expect("in-5-days segment must appear");
    assert_eq!(
        style.fg,
        Some(Color::Reset),
        "a due date outside the near window must carry no due color: {style:?}"
    );

    set_language("en");
}

#[test]
fn view_list_no_duedate_renders_no_due_date_in_default_style() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row("PROJ-6", "No due date issue", "Open", None, None);
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(buffer_text(&buf).contains("no due date"));
    let style = style_at_text(&buf, "no due date").expect("no-due-date segment must appear");
    assert_eq!(
        style.fg,
        Some(Color::Reset),
        "no duedate must render in the default style: {style:?}"
    );

    set_language("en");
}

// ---- BDR 0007 S4: whole-card selection + windowing ----

#[test]
fn view_list_selected_card_carries_selected_style_on_all_four_rows() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let rows = vec![
        make_card_row("PROJ-1", "First issue", "Open", None, None),
        make_card_row("PROJ-2", "Second issue", "Open", None, None),
    ];
    let model = make_list_model_with_rows(rows, 1);
    let buf = render_to_buffer(&model, 40, 20);

    // Content starts at row 1 (row 0 is the identity header); card 0 spans
    // rows 1-4, card 1 (selected) spans rows 5-8.
    for row in 5..=8u16 {
        for col in 0..buf.area.width {
            assert_bar_style(
                cell_style(&buf, col, row),
                Color::Rgb(13, 13, 13),
                Color::Rgb(210, 160, 90),
            );
        }
    }

    let unselected_has_selected_bg = (1..=4u16).any(|row| {
        (0..buf.area.width)
            .any(|col| cell_style(&buf, col, row).bg == Some(Color::Rgb(210, 160, 90)))
    });
    assert!(
        !unselected_has_selected_bg,
        "the non-selected card must carry no cell in the selected style"
    );

    set_language("en");
}

#[test]
fn view_list_windowing_keeps_selected_last_card_fully_visible() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let rows: Vec<IssueRow> = (1..=10)
        .map(|n| make_card_row(&format!("PROJ-{n}"), "An issue", "Open", None, None))
        .collect();
    let model = make_list_model_with_rows(rows, 9);

    // header(1) + content(13, 3 cards worth) + footer(1)
    let buf = render_to_buffer(&model, 40, 15);
    let text = buffer_text(&buf);

    assert!(
        text.contains("PROJ-10"),
        "the selected (last) card must scroll into view; got: {text}"
    );
    assert!(
        !text.contains("PROJ-1 "),
        "cards scrolled out of the window must not render; got: {text}"
    );

    set_language("en");
}

// ---- pure unit tests: card meta line + windowing helpers ----

#[test]
fn card_meta_line_omits_absent_project() {
    let row = make_card_row("PROJ-1", "Summary", "Open", None, None);
    assert_eq!(view::card_meta_line(&row, 0), "no due date · Open");
}

#[test]
fn card_meta_line_omits_empty_status() {
    let row = make_card_row("PROJ-1", "Summary", "", Some("Proj"), None);
    assert_eq!(view::card_meta_line(&row, 0), "no due date · Proj");
}

#[test]
fn card_meta_line_includes_all_segments_when_present() {
    let row = make_card_row("PROJ-1", "Summary", "Open", Some("Proj"), None);
    assert_eq!(view::card_meta_line(&row, 0), "no due date · Open · Proj");
}

#[test]
fn first_visible_card_fits_all_when_count_within_visible() {
    assert_eq!(view::first_visible_card(2, 3, 5), 0);
}

#[test]
fn first_visible_card_keeps_selected_in_window() {
    assert_eq!(view::first_visible_card(9, 10, 3), 7);
    assert_eq!(view::first_visible_card(0, 10, 3), 0);
    assert_eq!(view::first_visible_card(5, 10, 3), 3);
}

#[test]
fn first_visible_card_zero_visible_never_panics() {
    assert_eq!(view::first_visible_card(4, 10, 0), 0);
}

// ---- AC5: no contract drift — CLI table and agent_json list ignore the
// new TUI-only IssueRow fields ----

#[test]
fn render_issue_table_and_mine_list_object_ignore_duedate_and_project() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let base = make_card_row("PROJ-1", "Fix something", "Open", None, None);
    let with_new_fields = IssueRow {
        duedate: Some("2026-07-10".to_owned()),
        project: Some("Proj".to_owned()),
        ..base.clone()
    };

    let mut out_base = Vec::new();
    crate::render::render_issue_table(&mut out_base, std::slice::from_ref(&base));
    let mut out_new = Vec::new();
    crate::render::render_issue_table(&mut out_new, std::slice::from_ref(&with_new_fields));
    assert_eq!(
        out_base, out_new,
        "the CLI table must ignore duedate/project"
    );

    let json_base = crate::agent_json::mine_list_object("jql", std::slice::from_ref(&base));
    let json_new =
        crate::agent_json::mine_list_object("jql", std::slice::from_ref(&with_new_fields));
    assert_eq!(
        json_base, json_new,
        "the agent_json list must ignore duedate/project"
    );

    set_language("en");
}
