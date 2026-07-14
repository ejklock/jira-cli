use super::model::{
    header_line, FooterMode, Identity, ListOrigin, Selection, StatusKind, StatusMsg,
};
use super::panel;
use super::theme;
use super::view;
use super::*;

use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::{IssueRow, ProjectRow};
use crate::test_support::{duedate_offset_from_today, project_row};
use ratatui::{
    backend::TestBackend,
    layout::Rect,
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
        revalidating: false,
        selection: None,
        list_origin: ListOrigin::Mine,
        projects: vec![],
        projects_selected: 0,
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

/// Finds `needle`'s first rendered cell's `(column, row)` — mirrors
/// `tests/unit/tui.rs`'s helper of the same purpose; used by the ADR 0018
/// `detail_link_at` geometry tests to click exactly on a rendered token.
fn find_text_position(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    for row in 0..buf.area.height {
        let text = row_text(buf, row);
        if let Some(start) = text.find(needle) {
            let col = text[..start].chars().count() as u16;
            return Some((col, row));
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
        revalidating: false,
        selection: None,
        list_origin: ListOrigin::Mine,
        projects: vec![],
        projects_selected: 0,
    }
}

fn make_projects_model(projects: Vec<ProjectRow>) -> Model {
    let mut model = make_list_model(vec![]);
    model.screen = Screen::Projects;
    model.projects = projects;
    model
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

// ---- ADR 0016 / BDR 0008 S8: the dim "refreshing…" revalidating indicator ----

#[test]
fn view_list_header_shows_refreshing_indicator_while_revalidating() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(single_identity());
    model.revalidating = true;

    let buf = render_to_buffer(&model, 120, 20);
    let header = row_text(&buf, 0);

    assert!(
        header.starts_with("me@x.com · acme"),
        "the identity text must still render on the left; got: {header:?}"
    );
    assert!(
        header.trim_end().ends_with("refreshing…"),
        "the dim indicator must render on the header row's right side; got: {header:?}"
    );

    set_language("en");
}

#[test]
fn view_list_header_omits_refreshing_indicator_when_not_revalidating() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_list_model(single_identity());
    assert!(!model.revalidating);

    let buf = render_to_buffer(&model, 120, 20);

    assert!(
        !row_text(&buf, 0).contains("refreshing…"),
        "a non-revalidating header must show no indicator; got: {:?}",
        row_text(&buf, 0)
    );

    set_language("en");
}

#[test]
fn view_list_header_refreshing_indicator_pt_br_translates() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let mut model = make_list_model(single_identity());
    model.revalidating = true;

    let buf = render_to_buffer(&model, 120, 20);

    assert!(
        row_text(&buf, 0).contains("atualizando…"),
        "pt_BR must render the translated indicator; got: {:?}",
        row_text(&buf, 0)
    );

    set_language("en");
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

// ---- B1 mouse foundations / BDR 0009 S3-S5 — list_click_card hit test ----
// (single layout source: built on list_layout_chunks/first_visible_card/
// CARD_HEIGHT, so these tests double as geometry oracles for the resolver.)

#[test]
fn list_click_card_resolves_each_row_of_a_card_to_its_index() {
    let rows = vec![
        make_card_row("PROJ-1", "First issue", "Open", None, None),
        make_card_row("PROJ-2", "Second issue", "Open", None, None),
    ];
    let model = make_list_model_with_rows(rows, 0);
    let area = Rect::new(0, 0, 40, 20);

    // header(1) occupies row 0; card 0 spans rows 1-4.
    for y in 1..=4u16 {
        assert_eq!(
            view::list_click_card(&model, area, y),
            Some(0),
            "row {y} of card 0 must resolve to index 0"
        );
    }
    // card 1 spans rows 5-8.
    for y in 5..=8u16 {
        assert_eq!(
            view::list_click_card(&model, area, y),
            Some(1),
            "row {y} of card 1 must resolve to index 1"
        );
    }
}

#[test]
fn list_click_card_header_and_footer_rows_are_none() {
    let rows = vec![make_card_row("PROJ-1", "First issue", "Open", None, None)];
    let model = make_list_model_with_rows(rows, 0);
    let area = Rect::new(0, 0, 40, 20);

    assert_eq!(
        view::list_click_card(&model, area, 0),
        None,
        "the header row must resolve to None"
    );
    assert_eq!(
        view::list_click_card(&model, area, area.height - 1),
        None,
        "the footer row must resolve to None"
    );
}

#[test]
fn list_click_card_below_last_visible_card_is_none() {
    let rows = vec![make_card_row("PROJ-1", "First issue", "Open", None, None)];
    let model = make_list_model_with_rows(rows, 0);
    let area = Rect::new(0, 0, 40, 20);

    // Only one card (rows 1-4); the empty space below it in the cards chunk
    // must not resolve to a phantom row.
    assert_eq!(view::list_click_card(&model, area, 5), None);
}

#[test]
fn list_click_card_empty_list_is_none() {
    let model = make_list_model_with_rows(vec![], 0);
    let area = Rect::new(0, 0, 40, 20);

    assert_eq!(view::list_click_card(&model, area, 5), None);
}

#[test]
fn list_click_card_windowed_click_resolves_windowed_index_not_zero() {
    let rows: Vec<IssueRow> = (1..=10)
        .map(|n| make_card_row(&format!("PROJ-{n}"), "An issue", "Open", None, None))
        .collect();
    let model = make_list_model_with_rows(rows, 9);
    // header(1) + content(13, 3 cards worth) + footer(1) — mirrors
    // view_list_windowing_keeps_selected_last_card_fully_visible's geometry.
    let area = Rect::new(0, 0, 40, 15);

    // first_visible_card(9, 10, 3) == 7 (pinned by the test above); clicking
    // the first visible slot's row must resolve to 7, not 0.
    assert_eq!(view::list_click_card(&model, area, 1), Some(7));
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

// ---- add-updated-created-tui-detail: Details panel's Created/Updated rows
// mirror the CLI get output (render_issue_human) — conditional-push, so the
// row only appears when the field is Some ----

#[test]
fn view_detail_details_panel_renders_created_and_updated_rows_when_present() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        created: Some("2026-01-01T00:00:00.000+0000".to_owned()),
        updated: Some("2026-01-05T00:00:00.000+0000".to_owned()),
        ..crate::test_support::issue("PROJ-70")
    });

    let buf = render_to_buffer(&model, 100, 40);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Created: 2026-01-01T00:00:00.000+0000"),
        "the Details panel's Created row must show the raw timestamp; got: {text}"
    );
    assert!(
        text.contains("Updated: 2026-01-05T00:00:00.000+0000"),
        "the Details panel's Updated row must show the raw timestamp; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_details_panel_omits_created_and_updated_rows_when_absent() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        created: None,
        updated: None,
        ..crate::test_support::issue("PROJ-71")
    });

    let buf = render_to_buffer(&model, 100, 40);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("Created:"),
        "no Created row must render when issue.created is None; got: {text}"
    );
    assert!(
        !text.contains("Updated:"),
        "no Updated row must render when issue.updated is None; got: {text}"
    );

    set_language("en");
}

// ---- ADR 0018 / BDR 0010 S1, S4: inline '[url]' link token in the Description panel ----

fn make_issue_with_inline_link(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        description: Some(crate::test_support::doc(vec![
            crate::test_support::paragraph(vec![crate::test_support::marked_text(
                "read the docs",
                vec![crate::test_support::link_mark("https://example.com")],
            )]),
        ])),
        ..crate::test_support::issue(key)
    }
}

#[test]
fn view_detail_description_shows_visible_url_token_styled_anchor_text_plain() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_inline_link("PROJ-63"));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("read the docs [https://example.com]"),
        "the anchor text followed by the visible [url] token must appear; got: {text}"
    );

    let anchor_style =
        style_at_text(&buf, "read the docs").expect("anchor text must appear in the buffer");
    let token_style = style_at_text(&buf, "[https://example.com]")
        .expect("[url] token must appear in the buffer");

    assert!(
        !anchor_style.add_modifier.contains(Modifier::UNDERLINED),
        "anchor text must render as normal body text, no link style: {anchor_style:?}"
    );
    assert!(
        token_style.add_modifier.contains(Modifier::UNDERLINED),
        "the [url] token must carry the link style: {token_style:?}"
    );

    // ADR 0018 §6 (D-group parity): the token additionally carries the theme
    // link color; the anchor text stays body-colored (no fg override).
    assert_eq!(
        token_style.fg,
        theme::link().fg,
        "the [url] token must render with the theme link color: {token_style:?}"
    );
    assert_ne!(
        anchor_style.fg,
        theme::link().fg,
        "anchor text must not carry the theme link color: {anchor_style:?}"
    );

    set_language("en");
}

// ---- ADR 0018 §5 / BDR 0010 S5, S7, S8: detail_link_at geometry (single
// geometry source — recomputes the same compose path render_detail_panels
// draws) ----

fn make_issue_with_inline_link_and_comments(key: &str, count: usize) -> crate::models::Issue {
    let comments = (0..count)
        .map(|i| {
            crate::test_support::comment(
                None,
                Some("Alice"),
                &crate::test_support::plain_paragraph(&format!("comment body number {i}")),
                Some("2026-01-01"),
                None,
            )
        })
        .collect();
    crate::models::Issue {
        comments,
        ..make_issue_with_inline_link(key)
    }
}

#[test]
fn detail_link_at_on_the_url_token_column_returns_the_href() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_inline_link("PROJ-80"));

    let (width, height) = (120, 30);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);
    let (col, row) =
        find_text_position(&buf, "[https://example.com]").expect("the token must render");

    assert_eq!(
        view::detail_link_at(&model, area, col, row),
        Some("https://example.com".to_owned())
    );

    set_language("en");
}

#[test]
fn detail_link_at_on_anchor_text_or_chrome_is_none() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_inline_link("PROJ-81"));

    let (width, height) = (120, 30);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);

    let (anchor_col, anchor_row) =
        find_text_position(&buf, "read the docs").expect("anchor text must render");
    assert_eq!(
        view::detail_link_at(&model, area, anchor_col, anchor_row),
        None,
        "a modifier-click on plain anchor text must resolve to None"
    );

    let (details_col, details_row) =
        find_text_position(&buf, "Details").expect("the Details panel border/title must render");
    assert_eq!(
        view::detail_link_at(&model, area, details_col, details_row),
        None,
        "a modifier-click on a panel border/title must resolve to None"
    );

    assert_eq!(
        view::detail_link_at(&model, area, 0, 0),
        None,
        "a modifier-click on the header row (outside the content viewport) must resolve to None"
    );

    let (_, assignee_row) =
        find_text_position(&buf, "Assignee:").expect("the Assignee meta row must render");
    let blank_row = assignee_row + 2;
    assert_eq!(
        view::detail_link_at(&model, area, 5, blank_row),
        None,
        "a modifier-click on the blank separator row between panels must resolve to None"
    );

    set_language("en");
}

#[test]
fn detail_link_at_finds_the_token_at_its_scrolled_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_inline_link_and_comments("PROJ-82", 20));

    // height=17: the Details panel now carries Created/Updated rows in
    // addition to the pre-existing Title/Key/Status/Type/Assignee rows, so
    // the viewport must be tall enough for the description token to still
    // render unscrolled.
    let (width, height) = (100, 17);
    let area = Rect::new(0, 0, width, height);

    let buf_unscrolled = render_to_buffer(&model, width, height);
    let (col, row_unscrolled) = find_text_position(&buf_unscrolled, "[https://example.com]")
        .expect("the token must render unscrolled");

    model.detail_scroll = 2;
    let buf_scrolled = render_to_buffer(&model, width, height);
    let expected_row = row_unscrolled
        .checked_sub(2)
        .expect("the token must still be below the header after a scroll offset of 2");
    assert_eq!(
        find_text_position(&buf_scrolled, "[https://example.com]").map(|(_, r)| r),
        Some(expected_row),
        "sanity: a scroll offset of 2 must shift the token up by exactly 2 rows"
    );

    assert_eq!(
        view::detail_link_at(&model, area, col, expected_row),
        Some("https://example.com".to_owned()),
        "the token must resolve at its shifted row under a non-zero scroll offset"
    );

    set_language("en");
}

#[test]
fn detail_link_at_on_a_wrapped_url_fragment_returns_the_complete_href() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let href = format!(
        "https://example.com/{}MARKER{}",
        "a".repeat(120),
        "b".repeat(20)
    );
    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::doc(vec![
            crate::test_support::paragraph(vec![crate::test_support::marked_text(
                "docs",
                vec![crate::test_support::link_mark(&href)],
            )]),
        ])),
        ..crate::test_support::issue("PROJ-83")
    });

    let (width, height) = (40, 30);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);

    let (_, first_row) = find_text_position(&buf, "[https://example.com/a")
        .expect("the token's first wrapped fragment must render");
    let (marker_col, marker_row) =
        find_text_position(&buf, "MARKER").expect("a later wrapped fragment must render");

    assert!(
        marker_row > first_row,
        "the URL must actually wrap across rows for this test to exercise S7; first_row={first_row}, marker_row={marker_row}"
    );

    assert_eq!(
        view::detail_link_at(&model, area, marker_col, marker_row),
        Some(href),
        "a click on a later wrapped fragment must resolve the COMPLETE href"
    );

    set_language("en");
}

// ---- ADR 0020 / BDR 0012 S3-S8: the Attachments panel (after Comments,
// link-styled '[n] ↗ filename' rows carrying href, blank-row breathing room,
// italic/dim footnote, reachable-by-scroll, empty list renders no panel) ----

fn make_issue_with_attachments(
    key: &str,
    attachments: Vec<crate::models::Attachment>,
) -> crate::models::Issue {
    crate::models::Issue {
        attachments,
        ..crate::test_support::issue(key)
    }
}

fn make_issue_with_marked_attachments(
    key: &str,
    count: usize,
    last_filename: &str,
) -> crate::models::Issue {
    let attachments = (0..count)
        .map(|i| {
            let filename = if i + 1 == count {
                last_filename.to_owned()
            } else {
                format!("file-{i}.txt")
            };
            crate::test_support::attachment(
                &filename,
                &format!("https://example.com/{i}"),
                None,
                None,
            )
        })
        .collect();
    make_issue_with_attachments(key, attachments)
}

#[test]
fn view_detail_renders_attachments_panel_after_comments_with_link_styled_rows() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut issue = make_issue_with_description_and_two_comments("PROJ-100");
    issue.attachments = vec![
        crate::test_support::attachment("a.pdf", "https://example.com/a.pdf", None, None),
        crate::test_support::attachment("b.png", "https://example.com/b.png", None, None),
    ];
    let mut model = make_detail_model(vec![]);
    model.detail = Some(issue);

    let buf = render_to_buffer(&model, 100, 40);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Attachments (2)"),
        "the localized counted header must appear; got: {text}"
    );
    assert!(
        text.contains("[1] ↗ a.pdf"),
        "the first attachment row must appear; got: {text}"
    );
    assert!(
        text.contains("[2] ↗ b.png"),
        "the second attachment row must appear; got: {text}"
    );

    let comments_line = text
        .lines()
        .position(|l| l.contains("Comments (2)"))
        .expect("the Comments panel must render");
    let attachments_line = text
        .lines()
        .position(|l| l.contains("Attachments (2)"))
        .expect("the Attachments panel must render");
    assert!(
        attachments_line > comments_line,
        "the Attachments panel must render AFTER the Comments panel"
    );

    let row_style =
        style_at_text(&buf, "[1] ↗ a.pdf").expect("the first attachment row must render");
    assert_eq!(
        row_style.fg,
        theme::link().fg,
        "an attachment row must carry the theme link color: {row_style:?}"
    );
    assert!(
        row_style.add_modifier.contains(Modifier::UNDERLINED),
        "an attachment row must carry the theme link style: {row_style:?}"
    );

    set_language("en");
}

#[test]
fn view_detail_attachments_header_and_footnote_translate_under_pt_br() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_attachments(
        "PROJ-101",
        vec![crate::test_support::attachment(
            "a.pdf",
            "https://example.com/a.pdf",
            None,
            None,
        )],
    ));

    let buf = render_to_buffer(&model, 100, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Anexos (1)"),
        "pt_BR must translate the counted header; got: {text}"
    );
    assert!(
        text.contains("Ctrl/Cmd+clique abre um anexo"),
        "pt_BR must translate the footnote; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_attachments_panel_has_one_blank_row_between_rows_and_italic_dim_footnote_last() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_attachments(
        "PROJ-102",
        vec![
            crate::test_support::attachment("a.pdf", "https://example.com/a.pdf", None, None),
            crate::test_support::attachment("b.png", "https://example.com/b.png", None, None),
        ],
    ));

    let buf = render_to_buffer(&model, 100, 30);

    let (_, row1) =
        find_text_position(&buf, "[1] ↗ a.pdf").expect("the first attachment row must render");
    let (_, row2) =
        find_text_position(&buf, "[2] ↗ b.png").expect("the second attachment row must render");
    assert_eq!(
        row2,
        row1 + 2,
        "exactly one blank row must separate consecutive attachment rows"
    );
    let between_row = row_text(&buf, row1 + 1);
    assert!(
        !between_row.contains('↗')
            && !between_row.contains("a.pdf")
            && !between_row.contains("b.png"),
        "the row between two attachment rows must carry no attachment content (only box \
         border/padding); got: {between_row:?}"
    );

    let (_, footnote_row) = find_text_position(&buf, "Ctrl/Cmd+click opens an attachment")
        .expect("the footnote must render");
    assert_eq!(
        footnote_row,
        row2 + 1,
        "the footnote must be the panel's very next (last) content line after the last \
         attachment row"
    );

    let footnote_style = style_at_text(&buf, "Ctrl/Cmd+click opens an attachment")
        .expect("the footnote must render");
    assert!(
        footnote_style.add_modifier.contains(Modifier::ITALIC),
        "the footnote must render italic: {footnote_style:?}"
    );
    assert!(
        footnote_style.add_modifier.contains(Modifier::DIM),
        "the footnote must render dim: {footnote_style:?}"
    );

    set_language("en");
}

#[test]
fn view_detail_scroll_to_max_offset_exposes_the_last_attachment_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_marked_attachments(
        "PROJ-103",
        30,
        "LASTATTACHMENT",
    ));
    model.detail_scroll = u16::MAX;

    let buf = render_to_buffer(&model, 60, 15);
    let text = buffer_text(&buf);

    assert!(
        text.contains("LASTATTACHMENT"),
        "scrolling to the max offset must expose the final attachment row (BDR 0012 S5); \
         got: {text}"
    );

    set_language("en");
}

#[test]
fn detail_link_at_over_an_attachment_row_returns_its_content_url() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_attachments(
        "PROJ-104",
        vec![crate::test_support::attachment(
            "a.pdf",
            "https://example.com/a.pdf",
            None,
            None,
        )],
    ));

    let (width, height) = (100, 30);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);
    let (col, row) =
        find_text_position(&buf, "[1] ↗ a.pdf").expect("the attachment row must render");

    assert_eq!(
        view::detail_link_at(&model, area, col, row),
        Some("https://example.com/a.pdf".to_owned())
    );

    set_language("en");
}

#[test]
fn detail_link_at_over_the_attachments_header_blank_row_and_footnote_is_none() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_attachments(
        "PROJ-105",
        vec![
            crate::test_support::attachment("a.pdf", "https://example.com/a.pdf", None, None),
            crate::test_support::attachment("b.png", "https://example.com/b.png", None, None),
        ],
    ));

    let (width, height) = (100, 30);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);

    let (header_col, header_row) =
        find_text_position(&buf, "Attachments (2)").expect("the header must render");
    assert_eq!(
        view::detail_link_at(&model, area, header_col, header_row),
        None,
        "a modifier-click on the Attachments header must resolve to None"
    );

    let (row1_col, row1_row) =
        find_text_position(&buf, "[1] ↗ a.pdf").expect("the first attachment row must render");
    assert_eq!(
        view::detail_link_at(&model, area, row1_col, row1_row + 1),
        None,
        "a modifier-click on the blank separator row must resolve to None"
    );

    let (footnote_col, footnote_row) =
        find_text_position(&buf, "Ctrl/Cmd+click opens an attachment")
            .expect("the footnote must render");
    assert_eq!(
        view::detail_link_at(&model, area, footnote_col, footnote_row),
        None,
        "a modifier-click on the footnote must resolve to None"
    );

    set_language("en");
}

#[test]
fn detail_link_at_over_an_attachment_row_resolves_href_at_a_non_zero_scroll_offset() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_marked_attachments("PROJ-106", 20, "z.zip"));
    model.detail_scroll = u16::MAX;

    let (width, height) = (60, 12);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);
    let (col, row) = find_text_position(&buf, "z.zip")
        .expect("the last attachment row must be visible after scrolling to the max offset");

    assert_eq!(
        view::detail_link_at(&model, area, col, row),
        Some("https://example.com/19".to_owned()),
        "the last attachment's href must resolve at its scrolled row"
    );

    set_language("en");
}

#[test]
fn view_detail_with_no_attachments_renders_no_attachments_panel() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::test_support::issue("PROJ-107"));

    let buf = render_to_buffer(&model, 100, 30);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("Attachments"),
        "an issue with no attachments must render no Attachments header; got: {text}"
    );
    assert!(
        !text.contains("Ctrl/Cmd+click opens an attachment"),
        "an issue with no attachments must render no footnote; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_attachments_panel_adds_lines_that_push_content_past_the_viewport() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    // 4 numbered comments compose content that fits exactly within a 40-row
    // viewport with no scrollbar (baseline); adding the Attachments panel's
    // own lines (blank separator + border + row + footnote + border) must
    // push the SAME content past that viewport, proving the panel actually
    // appends composed lines rather than merely swapping displayed text.
    let base_issue = make_issue_with_numbered_comments("PROJ-108", 4, "LASTMARKER");
    let mut with_attachment = base_issue.clone();
    with_attachment.attachments = vec![crate::test_support::attachment(
        "a.pdf",
        "https://example.com/a.pdf",
        None,
        None,
    )];

    // height bumped from 40 to 42: the Details panel now carries two more
    // rows (Created/Updated), which grows the baseline by exactly that much.
    let (width, height) = (100, 42);
    let mut model_without = make_detail_model(vec![]);
    model_without.detail = Some(base_issue);
    let mut model_with = make_detail_model(vec![]);
    model_with.detail = Some(with_attachment);

    let text_without = buffer_text(&render_to_buffer(&model_without, width, height));
    let text_with = buffer_text(&render_to_buffer(&model_with, width, height));

    assert!(
        !text_without.contains('█'),
        "the baseline content must fit the viewport with no scrollbar; got: {text_without}"
    );
    assert!(
        text_with.contains('█'),
        "adding one attachment must grow the composed content enough to require scrolling; \
         got: {text_with}"
    );
    assert!(!text_without.contains("Attachments"));
    assert!(text_with.contains("Attachments (1)"));

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

// ---- b3-app-managed-selection / ADR 0019 §2 / BDR 0011 S5-S10 — detail_pos_at
// / detail_pos_at_clamped / selection_text geometry ----

#[test]
fn detail_pos_at_on_chrome_border_or_title_is_none() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_detail_model(vec![]);
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);

    let (details_col, details_row) =
        find_text_position(&buf, "Details").expect("the Details panel border/title must render");
    assert_eq!(
        view::detail_pos_at(&model, area, details_col, details_row),
        None,
        "a click on a panel border/title must resolve to no logical position (BDR 0011 S5)"
    );
    assert_eq!(
        view::detail_pos_at(&model, area, 0, 0),
        None,
        "a click on the header row (outside the content viewport) must resolve to None"
    );

    set_language("en");
}

#[test]
fn selection_text_over_a_word_extracts_exactly_that_word_no_chrome() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    // A description word unrelated to the issue summary/Title meta row (both
    // of which also render "A neutral issue ..." text), so the FIRST rendered
    // occurrence is unambiguously the Description panel's own body.
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::plain_paragraph("banana apple cherry")),
        ..crate::test_support::issue("PROJ-95")
    });
    let area = Rect::new(0, 0, 100, 20);
    let buf = render_to_buffer(&model, 100, 20);

    // Select exactly the middle word "apple" by anchoring on its first column
    // and ending at the column of the space right after it.
    let (word_col, word_row) =
        find_text_position(&buf, "apple").expect("the description body must render");
    let (space_col, space_row) =
        find_text_position(&buf, " cherry").expect("the following word must render");
    assert_eq!(
        word_row, space_row,
        "both positions must be on the same visual row"
    );

    let start = view::detail_pos_at(&model, area, word_col, word_row)
        .expect("the start of the word must resolve to a logical position");
    let end = view::detail_pos_at(&model, area, space_col, space_row)
        .expect("the column right after the word must resolve to a logical position");
    assert_eq!(
        start.0, end.0,
        "both positions must belong to the same logical line"
    );

    model.selection = Some(Selection {
        anchor: start,
        cursor: end,
        dragged: true,
    });

    assert_eq!(
        view::selection_text(&model),
        Some("apple".to_owned()),
        "the extracted text must be exactly the word, with no chrome (BDR 0011 S5)"
    );

    set_language("en");
}

#[test]
fn detail_pos_at_both_columns_of_a_double_width_glyph_map_to_the_same_char_index() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::plain_paragraph("Z你好Q")),
        ..crate::test_support::issue("PROJ-90")
    });

    let area = Rect::new(0, 0, 100, 20);
    let buf = render_to_buffer(&model, 100, 20);
    // A wide glyph's continuation cell renders a plain space in the buffer
    // (e.g. "Z你 好 Q"), so the needle can only span up to the first wide
    // glyph — "Z你" is still contiguous since Z (width 1) is immediately
    // followed by 你's own cell.
    let (col_z, row) = find_text_position(&buf, "Z你").expect("the description body must render");

    let pos_z = view::detail_pos_at(&model, area, col_z, row).expect("Z's column must resolve");
    let pos_ni_col1 =
        view::detail_pos_at(&model, area, col_z + 1, row).expect("你's first column must resolve");
    let pos_ni_col2 = view::detail_pos_at(&model, area, col_z + 2, row)
        .expect("你's second (continuation) column must resolve");
    let pos_hao =
        view::detail_pos_at(&model, area, col_z + 3, row).expect("好's first column must resolve");
    let pos_q = view::detail_pos_at(&model, area, col_z + 5, row).expect("Q's column must resolve");

    assert_eq!(
        pos_z.0, pos_ni_col1.0,
        "all positions must share the same logical line"
    );
    assert_eq!(
        pos_z.1 + 1,
        pos_ni_col1.1,
        "Z must be exactly one char before 你"
    );
    assert_eq!(
        pos_ni_col1, pos_ni_col2,
        "both display columns of a double-width glyph must map to the SAME char index, \
         never treating a column as a char index (BDR 0011 S6)"
    );
    assert_eq!(
        pos_ni_col1.1 + 1,
        pos_hao.1,
        "好 must be the very next char after 你, neither skipped nor duplicated"
    );
    assert_eq!(
        pos_hao.1 + 1,
        pos_q.1,
        "Q must be the very next char after 好"
    );

    set_language("en");
}

#[test]
fn selection_text_across_a_wrap_seam_is_contiguous_with_no_loss_or_duplication() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    // Wide enough to guarantee the leading run alone spans more than one
    // visual row, so the trailing run's first char necessarily lands on a
    // LATER row than the leading run's first char (a real wrap seam falls
    // somewhere inside the leading run itself).
    let leading = "X".repeat(60);
    let trailing = "Y".repeat(60);
    let body = format!("{leading}{trailing}");
    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::plain_paragraph(&body)),
        ..crate::test_support::issue("PROJ-92")
    });

    let (width, height) = (30, 40);
    let area = Rect::new(0, 0, width, height);
    let buf = render_to_buffer(&model, width, height);

    let (x_col, x_row) =
        find_text_position(&buf, "X").expect("the leading run's first row must render");
    let (y_col, y_row) =
        find_text_position(&buf, "Y").expect("the trailing run must render (possibly wrapped)");
    assert!(
        y_row > x_row,
        "the single unbroken word must actually wrap across visual rows for this test to \
         exercise S7 (a seam falling inside it); x_row={x_row}, y_row={y_row}"
    );

    let start = view::detail_pos_at(&model, area, x_col, x_row)
        .expect("the leading run's start must resolve");
    let end = view::detail_pos_at(&model, area, y_col, y_row)
        .expect("the trailing run's start must resolve");

    model.selection = Some(Selection {
        anchor: start,
        cursor: end,
        dragged: true,
    });

    assert_eq!(
        view::selection_text(&model),
        Some(leading),
        "a selection spanning a wrap seam (inside the single unbroken word) must yield the \
         contiguous pre-wrap logical text, with nothing dropped or repeated at the seam \
         (BDR 0011 S7)"
    );

    set_language("en");
}

#[test]
fn selection_text_is_identical_regardless_of_the_current_scroll_offset() {
    let mut model = make_detail_model(vec![]);
    model.detail = Some(make_issue_with_numbered_comments(
        "PROJ-93",
        30,
        "LASTMARKER",
    ));
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 5),
        dragged: true,
    });

    model.detail_scroll = 0;
    let unscrolled = view::selection_text(&model);

    model.detail_scroll = 25;
    let scrolled = view::selection_text(&model);

    assert_eq!(
        unscrolled, scrolled,
        "scrolling must never move a selection stored in logical coordinates (BDR 0011 S8)"
    );
    assert!(unscrolled.is_some());

    set_language("en");
}

#[test]
fn detail_pos_at_clamped_row_above_content_clamps_to_the_first_content_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_detail_model(vec![]);
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (_, title_row) = find_text_position(&buf, "Title:").expect("the Title row must render");

    let clamped = view::detail_pos_at_clamped(&model, area, 5, 0)
        .expect("a row above the content must still clamp to a position (BDR 0011 S10)");
    let direct = view::detail_pos_at_clamped(&model, area, 5, title_row)
        .expect("the Title row itself must resolve");

    assert_eq!(
        clamped, direct,
        "a coordinate above the content viewport must clamp down to the first content row"
    );

    set_language("en");
}

#[test]
fn detail_pos_at_clamped_row_below_content_clamps_to_the_last_content_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_detail_model(vec![]);
    let area = Rect::new(0, 0, 60, 20);

    let clamped_far_below = view::detail_pos_at_clamped(&model, area, 5, 9_999)
        .expect("a row far below the content must still clamp to a position (BDR 0011 S10)");
    let clamped_just_below_area = view::detail_pos_at_clamped(&model, area, 5, area.height - 1)
        .expect("the footer row must still clamp to a position");

    assert_eq!(
        clamped_far_below, clamped_just_below_area,
        "any row at or past the bottom of the viewport must clamp to the same last content row"
    );

    set_language("en");
}

#[test]
fn detail_pos_at_clamped_column_past_line_end_clamps_to_the_full_line_length() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (_, title_row) = find_text_position(&buf, "Title:").expect("the Title row must render");

    let (line, char_idx) = view::detail_pos_at_clamped(&model, area, 9_999, title_row)
        .expect("a far-right column must still clamp to a position (BDR 0011 S10)");

    model.selection = Some(Selection {
        anchor: (line, 0),
        cursor: (line, char_idx),
        dragged: true,
    });
    let extracted = view::selection_text(&model)
        .expect("the clamped end position must select the whole line's text");

    assert_eq!(
        extracted, "Title: A neutral issue summary",
        "a column far past the line's end must clamp to the FULL line, never truncating \
         or reading past it"
    );

    set_language("en");
}

#[test]
fn selection_text_spanning_two_logical_lines_joins_with_a_newline() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::doc(vec![
            crate::test_support::paragraph(vec![crate::test_support::text("Line one")]),
            crate::test_support::paragraph(vec![crate::test_support::text("Line two")]),
        ])),
        ..crate::test_support::issue("PROJ-91")
    });

    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (start_col, start_row) =
        find_text_position(&buf, "Line one").expect("the first paragraph must render");
    let (_, end_row) =
        find_text_position(&buf, "Line two").expect("the second paragraph must render");

    let (start_line, _) = view::detail_pos_at(&model, area, start_col, start_row)
        .expect("the start of the first paragraph must resolve");
    let (end_line, end_char) = view::detail_pos_at_clamped(&model, area, 9_999, end_row)
        .expect("a far-right column on the second paragraph's row must clamp to its end");

    model.selection = Some(Selection {
        anchor: (start_line, 0),
        cursor: (end_line, end_char),
        dragged: true,
    });

    assert_eq!(
        view::selection_text(&model),
        Some("Line one\nLine two".to_owned()),
        "a selection spanning two logical lines must join them with a newline"
    );

    set_language("en");
}

// ---- ADR 0019 §5 / BDR 0011 S1 — the REVERSED highlight over the exact
// covered cells, patched onto (not replacing) the underlying line ----

#[test]
fn render_detail_panels_highlights_exactly_the_selected_chars_reversed() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_detail_model(vec![]);
    model.detail = Some(crate::models::Issue {
        description: Some(crate::test_support::plain_paragraph("abcde")),
        ..crate::test_support::issue("PROJ-94")
    });

    let area = Rect::new(0, 0, 100, 20);
    let buf_before = render_to_buffer(&model, 100, 20);
    let (col_a, row) =
        find_text_position(&buf_before, "abcde").expect("the description body must render");

    // Selection is a half-open [start, end) char range: anchoring on 'b' and
    // ending at 'd' selects exactly "bc".
    let start = view::detail_pos_at(&model, area, col_a + 1, row).expect("'b' must resolve");
    let end = view::detail_pos_at(&model, area, col_a + 3, row).expect("'d' must resolve");

    model.selection = Some(Selection {
        anchor: start,
        cursor: end,
        dragged: true,
    });
    let buf_after = render_to_buffer(&model, 100, 20);

    assert!(
        !cell_style(&buf_after, col_a, row)
            .add_modifier
            .contains(Modifier::REVERSED),
        "'a' (before the selection) must not be highlighted"
    );
    assert!(
        cell_style(&buf_after, col_a + 1, row)
            .add_modifier
            .contains(Modifier::REVERSED),
        "'b' must be highlighted"
    );
    assert!(
        cell_style(&buf_after, col_a + 2, row)
            .add_modifier
            .contains(Modifier::REVERSED),
        "'c' must be highlighted"
    );
    assert!(
        !cell_style(&buf_after, col_a + 3, row)
            .add_modifier
            .contains(Modifier::REVERSED),
        "'d' (past the selection's end) must not be highlighted"
    );
    assert!(
        !cell_style(&buf_before, col_a + 1, row)
            .add_modifier
            .contains(Modifier::REVERSED),
        "with no selection, 'b' must render with no REVERSED highlight"
    );

    set_language("en");
}

// ---- ADR 0021 / BDR 0013 — Projects screen rendering ----

#[test]
fn view_projects_renders_title_rows_and_footer_hint() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_projects_model(vec![
        project_row("ALPHA", "Alpha Project"),
        project_row("BETA", "Beta Project"),
    ]);
    let buf = render_to_buffer(&model, 80, 20);
    let text = buffer_text(&buf);

    assert!(text.contains("Projects"), "title must render; got: {text}");
    assert!(
        text.contains("ALPHA — Alpha Project"),
        "first row must render as 'KEY — name'; got: {text}"
    );
    assert!(
        text.contains("BETA — Beta Project"),
        "second row must render as 'KEY — name'; got: {text}"
    );
    assert!(
        row_text(&buf, 19).contains("↑/↓ navigate  Enter select  Esc/b back  q quit"),
        "footer hint must be the Projects mode hint; got: {:?}",
        row_text(&buf, 19)
    );

    set_language("en");
}

#[test]
fn view_projects_styles_the_selected_row() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_projects_model(vec![
        project_row("ALPHA", "Alpha Project"),
        project_row("BETA", "Beta Project"),
    ]);
    model.projects_selected = 1;

    let buf = render_to_buffer(&model, 80, 20);

    let selected_style =
        style_at_text(&buf, "BETA — Beta Project").expect("selected row must render");
    assert_eq!(selected_style.fg, Some(Color::Rgb(13, 13, 13)));
    assert_eq!(selected_style.bg, Some(Color::Rgb(210, 160, 90)));
    assert!(selected_style.add_modifier.contains(Modifier::BOLD));

    let unselected_style =
        style_at_text(&buf, "ALPHA — Alpha Project").expect("unselected row must render");
    assert_ne!(
        unselected_style.bg,
        Some(Color::Rgb(210, 160, 90)),
        "the unselected row must not carry the selected-row background"
    );

    set_language("en");
}

#[test]
fn view_projects_empty_shows_localized_empty_state_en() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_projects_model(vec![]);
    let buf = render_to_buffer(&model, 80, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("No projects."),
        "empty projects must show the localized empty-state notice; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_projects_pt_br_title_hint_and_empty_state() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let model = make_projects_model(vec![]);
    let buf = render_to_buffer(&model, 80, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Projetos"),
        "the pt-BR title must be localized; got: {text}"
    );
    assert!(
        text.contains("Nenhum projeto encontrado."),
        "the pt-BR empty state must be localized; got: {text}"
    );
    assert!(
        row_text(&buf, 19).contains("↑/↓ navegar  Enter selecionar  Esc/b voltar  q sair"),
        "the pt-BR footer hint must be localized; got: {:?}",
        row_text(&buf, 19)
    );

    set_language("en");
}

// ---- ADR 0017 single-layout-source — projects_click_row hit test ----

#[test]
fn projects_click_row_resolves_each_row_to_its_index() {
    let model = make_projects_model(vec![
        project_row("ALPHA", "Alpha Project"),
        project_row("BETA", "Beta Project"),
        project_row("GAMMA", "Gamma Project"),
    ]);
    let area = Rect::new(0, 0, 40, 20);

    // header(1) + title(1) => rows start at y=2, one row per project.
    assert_eq!(view::projects_click_row(&model, area, 2), Some(0));
    assert_eq!(view::projects_click_row(&model, area, 3), Some(1));
    assert_eq!(view::projects_click_row(&model, area, 4), Some(2));
}

#[test]
fn projects_click_row_header_and_title_rows_are_none() {
    let model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    let area = Rect::new(0, 0, 40, 20);

    assert_eq!(view::projects_click_row(&model, area, 0), None);
    assert_eq!(view::projects_click_row(&model, area, 1), None);
}

#[test]
fn projects_click_row_below_last_row_is_none() {
    let model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    let area = Rect::new(0, 0, 40, 20);

    assert_eq!(view::projects_click_row(&model, area, 3), None);
}

#[test]
fn projects_click_row_empty_projects_is_none() {
    let model = make_projects_model(vec![]);
    let area = Rect::new(0, 0, 40, 20);

    assert_eq!(view::projects_click_row(&model, area, 2), None);
}

#[test]
fn projects_click_row_windowed_click_resolves_windowed_index_not_zero() {
    let projects: Vec<ProjectRow> = (1..=10)
        .map(|n| project_row(&format!("PROJ{n}"), &format!("Project {n}")))
        .collect();
    let mut model = make_projects_model(projects);
    model.projects_selected = 9;
    // header(1) + title(1) + rows(15) + footer(1) => 18 rows of content.
    let area = Rect::new(0, 0, 40, 18);

    // first_visible_card(9, 10, 15) == 0 (all fit), so a click on the last
    // row (index 9 => y = 2 + 9 = 11) must resolve to 9, not clamp away.
    assert_eq!(view::projects_click_row(&model, area, 11), Some(9));
}
