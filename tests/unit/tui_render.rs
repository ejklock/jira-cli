use super::model::{header_line, FooterMode, Identity, StatusKind, StatusMsg};
use super::panel;
use super::theme;
use super::view;
use super::*;

use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::IssueRow;
use crate::test_support::duedate_offset_from_today;
use ratatui::{
    backend::TestBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
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
        status: None,
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
        status: None,
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

// ---- issue 0033 D4 / BDR 0007 S7: footer_hint — one string per FooterMode ----

#[test]
fn footer_hint_en_exact_strings_per_mode() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    assert_eq!(
        view::footer_hint(FooterMode::List),
        "↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"
    );
    assert_eq!(
        view::footer_hint(FooterMode::ListSearch),
        "Enter submit  Esc cancel  Backspace delete"
    );
    assert_eq!(
        view::footer_hint(FooterMode::Detail),
        "↑/↓ j/k scroll  Esc/b back  q quit"
    );
    assert_eq!(
        view::footer_hint(FooterMode::DetailLink),
        "↑/↓ j/k scroll  Tab next link  Enter open  Esc/b back  q quit"
    );

    set_language("en");
}

#[test]
fn footer_hint_pt_br_exact_strings_per_mode() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    assert_eq!(
        view::footer_hint(FooterMode::List),
        "↑/↓ navegar  /  buscar  Enter selecionar  Esc/b voltar  q sair"
    );
    assert_eq!(
        view::footer_hint(FooterMode::ListSearch),
        "Enter enviar  Esc cancelar  Backspace apagar"
    );
    assert_eq!(
        view::footer_hint(FooterMode::Detail),
        "↑/↓ j/k rolar  Esc/b voltar  q sair"
    );
    assert_eq!(
        view::footer_hint(FooterMode::DetailLink),
        "↑/↓ j/k rolar  Tab próximo link  Enter abrir  Esc/b voltar  q sair"
    );

    set_language("en");
}

// ---- issue 0033 D4 / BDR 0007 S8: thin transient status row above the footer ----

#[test]
fn view_list_status_row_renders_info_confirmation_above_footer() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(vec![]);
    model.status = Some(StatusMsg {
        text: "Copied ✓".to_owned(),
        kind: StatusKind::Info,
    });

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Copied ✓"),
        "the status row must show the Info confirmation; got: {text}"
    );
    let style = style_at_text(&buf, "Copied ✓").expect("status text must appear in buffer");
    assert_bar_style(style, Color::Rgb(208, 216, 224), Color::Rgb(38, 52, 74));

    set_language("en");
}

#[test]
fn view_list_status_row_renders_error_style_for_a_fetch_error() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(vec![]);
    model.status = Some(StatusMsg {
        text: "network unreachable".to_owned(),
        kind: StatusKind::Error,
    });

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("network unreachable"),
        "the status row must show the fetch error; got: {text}"
    );
    let style =
        style_at_text(&buf, "network unreachable").expect("status text must appear in buffer");
    assert_bar_style(style, Color::Rgb(224, 108, 108), Color::Rgb(38, 52, 74));

    set_language("en");
}

#[test]
fn view_list_with_no_status_reserves_no_status_row_and_layout_stays_collapsed() {
    let model = make_list_model(vec![]);

    let buf = render_to_buffer(&model, 120, 20);

    assert_bar_style(
        cell_style(&buf, 0, 19),
        Color::Rgb(208, 216, 224),
        Color::Rgb(38, 52, 74),
    );
}

#[test]
fn view_list_status_row_clears_on_the_next_key_event_and_layout_collapses_back() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let with_status = {
        let mut model = make_list_model(vec![]);
        model.status = Some(StatusMsg {
            text: "Copied ✓".to_owned(),
            kind: StatusKind::Info,
        });
        model
    };
    let (cleared, _) = update(with_status, Msg::Down);

    assert!(
        cleared.status.is_none(),
        "the next key event must clear the status row"
    );

    let buf = render_to_buffer(&cleared, 120, 20);
    let text = buffer_text(&buf);
    assert!(
        !text.contains("Copied ✓"),
        "the status row must not render once cleared; got: {text}"
    );
    assert_bar_style(
        cell_style(&buf, 0, 19),
        Color::Rgb(208, 216, 224),
        Color::Rgb(38, 52, 74),
    );

    set_language("en");
}

#[test]
fn view_detail_status_row_renders_above_the_footer() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.status = Some(StatusMsg {
        text: "Copied ✓".to_owned(),
        kind: StatusKind::Info,
    });

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Copied ✓"),
        "the detail screen's status row must show the confirmation; got: {text}"
    );
    assert!(
        row_text(&buf, 29).contains("↑/↓ j/k scroll  Esc/b back  q quit"),
        "the footer must still render below the status row; got: {:?}",
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

// D2 review follow-up (issue 0033): a delta-of-exactly-2 due date is still
// inside the near window ((0..=2), not (0..=1)) — kills the boundary mutant.
#[test]
fn view_list_due_in_exactly_two_days_renders_near_amber() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let row = make_card_row(
        "PROJ-8",
        "Boundary due issue",
        "Open",
        None,
        Some(duedate_offset_from_today(2)),
    );
    // selected=1 with a single row (index 0) so this card renders unselected —
    // the assertions below probe its own due-color style, not the selection
    // override.
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(buffer_text(&buf).contains("in 2 days"));
    let style = style_at_text(&buf, "in 2 days").expect("in-2-days segment must appear");
    assert_eq!(
        style.fg,
        Some(Color::Rgb(210, 160, 90)),
        "a due date exactly 2 days out is still inside the near window: {style:?}"
    );

    set_language("en");
}

// D2 review follow-up (issue 0033): the "no due date" catalog key was missing
// from pt_BR (t() fell back to the untranslated English text).
#[test]
fn view_list_no_duedate_pt_br_translates_to_sem_data() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let row = make_card_row("PROJ-9", "No due date issue", "Open", None, None);
    let model = make_list_model_with_rows(vec![row], 1);
    let buf = render_to_buffer(&model, 60, 20);

    assert!(
        buffer_text(&buf).contains("sem data"),
        "pt_BR must translate 'no due date' to 'sem data'"
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

// ---- issue 0032 D3 / BDR 0007 S5-S6: detail as stacked rounded panels ----

fn make_issue_with_description_and_two_comments(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        summary: "Fix the login flow".to_owned(),
        status: "In Progress".to_owned(),
        status_category: None,
        issue_type: "Bug".to_owned(),
        assignee: Some(crate::test_support::assignee("Alice", None)),
        description: Some(crate::test_support::plain_paragraph("A description body.")),
        comments: vec![
            crate::test_support::comment(
                None,
                Some("Alice"),
                &crate::test_support::plain_paragraph("First comment."),
                Some("2026-01-01"),
                None,
            ),
            crate::test_support::comment(
                None,
                Some("Bob"),
                &crate::test_support::plain_paragraph("Second comment."),
                Some("2026-01-02"),
                None,
            ),
        ],
        ..crate::test_support::issue(key)
    }
}

fn make_issue_with_numbered_comments(
    key: &str,
    count: usize,
    last_marker: &str,
) -> crate::models::Issue {
    let comments = (0..count)
        .map(|i| {
            let body = if i + 1 == count {
                last_marker.to_owned()
            } else {
                format!("comment body number {i}")
            };
            crate::test_support::comment(
                None,
                Some("Alice"),
                &crate::test_support::plain_paragraph(&body),
                Some("2026-01-01"),
                None,
            )
        })
        .collect();
    crate::models::Issue {
        comments,
        ..crate::test_support::issue(key)
    }
}

#[test]
fn view_detail_renders_three_panels_with_details_meta_rows() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_description_and_two_comments("PROJ-60"));

    let buf = render_to_buffer(&model, 100, 40);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Details"),
        "the Details panel title must appear; got: {text}"
    );
    assert!(
        text.contains("Description"),
        "the Description panel title must appear; got: {text}"
    );
    assert!(
        text.contains("Comments (2)"),
        "the Comments panel title must include the comment count; got: {text}"
    );
    assert!(
        text.contains("Fix the login flow"),
        "the frame border title must show the issue summary; got: {text}"
    );
    assert!(
        text.contains("PROJ-60"),
        "the Details panel's Key row must appear; got: {text}"
    );
    assert!(
        text.contains("In Progress"),
        "the Details panel's Status row must appear; got: {text}"
    );
    assert!(
        text.contains("Bug"),
        "the Details panel's Type row must appear; got: {text}"
    );
    assert!(
        text.contains("Alice"),
        "the Details panel's Assignee row must appear; got: {text}"
    );
    assert!(
        text.contains("A description body."),
        "the Description panel body must appear; got: {text}"
    );
    assert!(
        text.contains("First comment."),
        "the first comment's body must appear; got: {text}"
    );
    assert!(
        text.contains("Second comment."),
        "the second comment's body must appear; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_border_title_ellipsizes_a_long_summary() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let long_summary = "A".repeat(200);
    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        summary: long_summary.clone(),
        ..crate::test_support::issue("PROJ-61")
    });

    let buf = render_to_buffer(&model, 40, 20);

    assert!(
        row_text(&buf, 1).contains('…'),
        "the border title must ellipsize a summary longer than the frame width; got: {:?}",
        row_text(&buf, 1)
    );
    assert!(
        !row_text(&buf, 1).contains(&long_summary),
        "the full over-long summary must not render verbatim; got: {:?}",
        row_text(&buf, 1)
    );

    set_language("en");
}

#[test]
fn view_detail_border_title_falls_back_to_key_when_summary_is_empty() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        summary: String::new(),
        ..crate::test_support::issue("PROJ-62")
    });

    let buf = render_to_buffer(&model, 100, 30);

    assert!(
        row_text(&buf, 1).contains("PROJ-62"),
        "an empty summary must fall back to the issue key in the border title; got: {:?}",
        row_text(&buf, 1)
    );

    set_language("en");
}

#[test]
fn view_detail_renders_scrollbar_when_content_exceeds_viewport() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_numbered_comments(
        "PROJ-70",
        30,
        "LASTMARKER",
    ));

    let buf = render_to_buffer(&model, 60, 15);
    let text = buffer_text(&buf);

    assert!(
        text.contains('█'),
        "a scrollbar thumb must render when content exceeds the viewport; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_renders_no_scrollbar_when_content_fits() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::test_support::issue("PROJ-71"));

    let buf = render_to_buffer(&model, 100, 60);
    let text = buffer_text(&buf);

    assert!(
        !text.contains('█'),
        "no scrollbar must render when all content fits the viewport; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_scroll_past_end_clamps_last_line_to_the_bottom_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_numbered_comments(
        "PROJ-72",
        30,
        "LASTMARKER",
    ));
    model.detail_scroll = u16::MAX;

    let height = 15u16;
    let buf = render_to_buffer(&model, 60, height);
    let text = buffer_text(&buf);

    assert!(
        text.contains("LASTMARKER"),
        "an extreme scroll offset must clamp to the last page instead of blank overscroll; got: {text}"
    );

    // header(1 row) + block top border(1 row) + block bottom border(1 row);
    // the remaining rows are the scrolled content. The very last row must be
    // the outer Comments panel's own closing border (never blank overscroll),
    // and the last comment's marker — a couple of lines above it, inside its
    // own nested card's closing border — must still be visible in the same
    // clamped page.
    let last_inner_row = height - 3;
    assert!(
        !row_text(&buf, last_inner_row).trim().is_empty(),
        "the last visible row must not be blank (no overscroll past the end); got: {:?}",
        row_text(&buf, last_inner_row)
    );
    let bottom_rows = (last_inner_row.saturating_sub(3)..=last_inner_row)
        .map(|row| row_text(&buf, row))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        bottom_rows.contains("LASTMARKER"),
        "the last comment's marker must be visible near the bottom of the clamped view; got: {bottom_rows:?}"
    );

    set_language("en");
}

// ---- issue 0032 D3: panel_box / fit_to_display_width / ellipsize_display geometry ----

fn line_display_width(line: &Line) -> usize {
    line.spans
        .iter()
        .map(|span| unicode_width::UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

fn line_text(line: &Line) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn panel_box_every_line_is_exactly_the_requested_width_with_wide_glyphs() {
    let body = vec![
        Line::from("plain row"),
        Line::from("你好嗎世界"),
        Line::from(""),
    ];
    let lines = panel::panel_box("Details", body, 24);

    for line in &lines {
        assert_eq!(
            line_display_width(line),
            24,
            "every panel_box line must be exactly 24 display columns; got: {:?}",
            line_text(line)
        );
    }
}

#[test]
fn panel_box_label_is_embedded_in_the_top_border() {
    let lines = panel::panel_box("Comments (2)", vec![Line::from("body")], 30);

    assert!(
        line_text(&lines[0]).contains("Comments (2)"),
        "the label must appear in the top border; got: {:?}",
        line_text(&lines[0])
    );
}

#[test]
fn panel_box_empty_body_still_emits_top_and_bottom_border_only() {
    let lines = panel::panel_box("Empty", vec![], 20);

    assert_eq!(
        lines.len(),
        2,
        "an empty body must still yield exactly a top and bottom border"
    );
    for line in &lines {
        assert_eq!(line_display_width(line), 20);
    }
}

#[test]
fn panel_box_label_longer_than_width_is_ellipsized_without_panic() {
    let long_label = "A".repeat(100);
    let lines = panel::panel_box(&long_label, vec![], 12);

    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert_eq!(line_display_width(line), 12);
    }
    assert!(
        line_text(&lines[0]).contains('…'),
        "an over-long label must be ellipsized; got: {:?}",
        line_text(&lines[0])
    );
}

#[test]
fn panel_box_preserves_styled_spans_in_body() {
    let styled_line = Line::from(vec![Span::styled(
        "bold run",
        Style::default().add_modifier(Modifier::BOLD),
    )]);
    let lines = panel::panel_box("Label", vec![styled_line], 30);

    let styled_span = lines[1]
        .spans
        .iter()
        .find(|span| span.content.as_ref() == "bold run")
        .expect("the styled body span must survive boxing");
    assert!(
        styled_span.style.add_modifier.contains(Modifier::BOLD),
        "the body span's BOLD style must survive boxing: {:?}",
        styled_span.style
    );
}

#[test]
fn fit_to_display_width_pads_a_short_line_to_exact_width() {
    let fitted = panel::fit_to_display_width(&Line::from("hi"), 6);

    assert_eq!(line_display_width(&fitted), 6);
    assert_eq!(line_text(&fitted), "hi    ");
}

#[test]
fn fit_to_display_width_truncates_a_long_line_preserving_style() {
    let line = Line::from(vec![Span::styled(
        "a very long styled run",
        Style::default().add_modifier(Modifier::BOLD),
    )]);
    let fitted = panel::fit_to_display_width(&line, 6);

    assert_eq!(line_display_width(&fitted), 6);
    assert_eq!(line_text(&fitted), "a very");
    assert!(
        fitted.spans[0].style.add_modifier.contains(Modifier::BOLD),
        "the truncated span must keep its BOLD style: {:?}",
        fitted.spans[0].style
    );
}

#[test]
fn fit_to_display_width_pads_the_leftover_column_instead_of_splitting_a_wide_glyph() {
    let fitted = panel::fit_to_display_width(&Line::from("你好嗎"), 5);

    assert_eq!(line_display_width(&fitted), 5);
    assert_eq!(line_text(&fitted), "你好 ");
}

#[test]
fn ellipsize_display_returns_text_unchanged_when_it_already_fits() {
    assert_eq!(panel::ellipsize_display("short", 10), "short");
}

#[test]
fn ellipsize_display_truncates_with_a_single_ellipsis_column() {
    assert_eq!(
        panel::ellipsize_display("a long piece of text", 6),
        "a lon…"
    );
}

#[test]
fn ellipsize_display_with_zero_columns_is_empty() {
    assert_eq!(panel::ellipsize_display("anything", 0), "");
}

// ---- issue 0034 / ADR 0014 §6 / BDR 0007 S9: ADF table in the Description panel ----

fn table_description_adf() -> String {
    use crate::test_support::{doc, paragraph, table, table_cell, table_header, table_row, text};
    doc(vec![table(vec![
        table_row(vec![
            table_header(vec![paragraph(vec![text("Name")])]),
            table_header(vec![paragraph(vec![text("Status")])]),
        ]),
        table_row(vec![
            table_cell(vec![paragraph(vec![text("Alice")])]),
            table_cell(vec![paragraph(vec![text("Open")])]),
        ]),
        table_row(vec![
            table_cell(vec![paragraph(vec![text("Bob")])]),
            table_cell(vec![paragraph(vec![text("Done")])]),
        ]),
    ])])
}

#[test]
fn view_detail_description_table_renders_one_line_per_row_with_bold_header() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(table_description_adf()),
        ..crate::test_support::issue("PROJ-80")
    });

    let buf = render_to_buffer(&model, 100, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Name │ Status"),
        "the header row must render as one line with the ' │ ' separator; got: {text}"
    );
    assert!(
        text.contains("Alice │ Open"),
        "the first data row must render with no dropped cell text; got: {text}"
    );
    assert!(
        text.contains("Bob │ Done"),
        "the second data row must render with no dropped cell text; got: {text}"
    );

    let header_style = style_at_text(&buf, "Name").expect("header cell text must appear");
    assert!(
        header_style.add_modifier.contains(Modifier::BOLD),
        "header row cells must render bold: {header_style:?}"
    );
    let data_style = style_at_text(&buf, "Alice").expect("data cell text must appear");
    assert!(
        !data_style.add_modifier.contains(Modifier::BOLD),
        "data row cells must not render bold: {data_style:?}"
    );

    set_language("en");
}
