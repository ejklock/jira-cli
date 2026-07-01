use super::*;

use crate::cli::{browse_tty_action, BrowseAction};
use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::IssueRow;
use ratatui::{backend::TestBackend, Terminal};

// ---- Helpers ----

// ADF fixture builders — assemble ADF-JSON via `serde_json::json!` instead of
// repeating the full `{"type":"doc","version":1,"content":[...]}` scaffolding
// as string literals in each issue fixture below.

fn doc(content: Vec<serde_json::Value>) -> String {
    serde_json::json!({"type": "doc", "version": 1, "content": content}).to_string()
}

fn paragraph(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "paragraph", "content": content})
}

fn text(value: &str) -> serde_json::Value {
    serde_json::json!({"type": "text", "text": value})
}

fn marked_text(value: &str, marks: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "text", "text": value, "marks": marks})
}

fn mark(mark_type: &str) -> serde_json::Value {
    serde_json::json!({"type": mark_type})
}

fn link_mark(href: &str) -> serde_json::Value {
    serde_json::json!({"type": "link", "attrs": {"href": href}})
}

fn make_test_instance() -> crate::store::instances::Instance {
    crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: "https://test.atlassian.net".to_owned(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    }
}

fn make_row(key: &str) -> IssueRow {
    IssueRow {
        key: key.to_owned(),
        issue_type: "Task".to_owned(),
        status: "Open".to_owned(),
        assignee: Some("Alice".to_owned()),
        summary: "Fix something".to_owned(),
    }
}

fn make_rows(keys: &[&str]) -> Vec<IssueRow> {
    keys.iter().map(|k| make_row(k)).collect()
}

fn make_list_model(keys: &[&str]) -> Model {
    Model {
        rows: make_rows(keys),
        selected: 0,
        screen: Screen::List,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: "https://test.atlassian.net".to_owned(),
        jql: "assignee = currentUser()".to_owned(),
        next_page_token: None,
    }
}

fn make_issue(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        key: key.to_owned(),
        summary: "Summary of the issue".to_owned(),
        status: "In Progress".to_owned(),
        status_category: Some("indeterminate".to_owned()),
        issue_type: "Bug".to_owned(),
        assignee: Some(crate::models::IssueAssignee {
            display_name: "Alice".to_owned(),
            account_id: None,
        }),
        reporter: None,
        priority: None,
        created: None,
        updated: None,
        description: Some(doc(vec![paragraph(vec![text(
            "Flattened description here.",
        )])])),
        comments: vec![],
    }
}

fn open_in_memory_store() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;\
         PRAGMA foreign_keys=ON;\
         CREATE TABLE IF NOT EXISTS issue_cache (
             instance_name TEXT NOT NULL,
             issue_key     TEXT NOT NULL,
             project_key   TEXT NOT NULL,
             fields_json   TEXT NOT NULL,
             fetched_at    TEXT NOT NULL,
             PRIMARY KEY (instance_name, issue_key)
         );",
    )
    .unwrap();
    conn
}

fn render_to_buffer(model: &Model, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(model, frame)).unwrap();
    terminal.backend().buffer().clone()
}

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let (width, height) = (buf.area.width as usize, buf.area.height as usize);
    (0..height)
        .map(|row| {
            (0..width)
                .map(|col| buf[(col as u16, row as u16)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn style_at_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<ratatui::style::Style> {
    let (width, height) = (buf.area.width as usize, buf.area.height as usize);
    for row in 0..height {
        let row_text: String = (0..width)
            .map(|col| buf[(col as u16, row as u16)].symbol().to_owned())
            .collect();
        if let Some(start) = row_text.find(needle) {
            return Some(buf[(start as u16, row as u16)].style());
        }
    }
    None
}

fn make_issue_with_styled_description(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        description: Some(doc(vec![paragraph(vec![
            marked_text("Bold text", vec![mark("strong")]),
            text(" plain then "),
            marked_text("a link", vec![link_mark("https://example.com")]),
        ])])),
        ..make_issue(key)
    }
}

fn build_search_payload_with_key(key: &str) -> serde_json::Value {
    serde_json::json!({
        "issues": [
            {
                "id": "10001",
                "key": key,
                "self": "https://example.atlassian.net/rest/api/3/issue/10001",
                "fields": {
                    "summary": "Search result issue",
                    "status": {
                        "id": "1",
                        "name": "Open",
                        "description": "",
                        "iconUrl": "",
                        "self": "",
                        "statusCategory": {
                            "id": 2,
                            "key": "new",
                            "colorName": "blue-gray",
                            "name": "To Do"
                        }
                    },
                    "issuetype": {
                        "id": "10002",
                        "name": "Task",
                        "description": "",
                        "iconUrl": "",
                        "self": "",
                        "subtask": false
                    },
                    "assignee": {
                        "accountId": "u1",
                        "displayName": "Bob",
                        "active": true,
                        "self": "",
                        "avatarUrls": {}
                    },
                    "priority": {
                        "id": "3",
                        "name": "Medium",
                        "iconUrl": "",
                        "self": ""
                    },
                    "created": "2026-01-01T00:00:00.000+0000",
                    "updated": "2026-06-29T00:00:00.000+0000"
                }
            }
        ],
        "isLast": true,
        "nextPageToken": null
    })
}

// ---- B0 tests (keep) ----

// ---- AC1 (B0): pure TTY-routing helper ----

#[test]
fn browse_tty_action_tty_yields_run_tui() {
    assert_eq!(browse_tty_action(true), BrowseAction::RunTui);
}

#[test]
fn browse_tty_action_non_tty_yields_tty_error() {
    assert_eq!(browse_tty_action(false), BrowseAction::TtyError);
}

// ---- AC2 (B0): non-TTY browse guard ----

#[tokio::test]
async fn non_tty_browse_writes_tty_error_to_stderr() {
    let conn = open_in_memory_store();
    let cache = crate::store::cache::TaskCache::new(&conn);
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, &cache, false, &mut stderr).await;

    let output = String::from_utf8(stderr).expect("utf8");
    assert!(
        output.contains("Error: 'browse' requires an interactive terminal (TTY)."),
        "expected TTY error in stderr, got: {output:?}"
    );
    assert_ne!(code, 0, "non-TTY browse must return a non-zero exit code");
}

#[tokio::test]
async fn non_tty_browse_returns_non_zero_without_any_network_call() {
    // No client is constructed on the guard path; if browse attempts to access the
    // network the test would hang or panic because no mock server is running.
    let conn = open_in_memory_store();
    let cache = crate::store::cache::TaskCache::new(&conn);
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, &cache, false, &mut stderr).await;

    assert_ne!(code, 0);
}

// ---- B1: AC1 — update Down/Up movement and clamping ----

#[test]
fn update_down_increments_selected() {
    let model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(next.selected, 1);
    assert!(cmds.is_empty());
}

#[test]
fn update_down_clamps_at_last_row() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.selected = 1;
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(next.selected, 1, "Down at last row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_up_decrements_selected() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    model.selected = 2;
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(next.selected, 1);
    assert!(cmds.is_empty());
}

#[test]
fn update_up_clamps_at_zero() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(next.selected, 0, "Up at first row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_down_on_empty_rows_is_noop() {
    let model = make_list_model(&[]);
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(
        next.selected, 0,
        "Down on empty list must not panic or change selected"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_empty_rows_is_noop() {
    let model = make_list_model(&[]);
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(
        next.selected, 0,
        "Up on empty list must not panic or change selected"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_preserves_rows_on_navigation() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let (next, _) = update(model, Msg::Down);
    assert_eq!(next.rows.len(), 2, "rows must be preserved through update");
    assert_eq!(next.rows[0].key, "PROJ-1");
}

// ---- B1: AC2 — update Quit emits Cmd::Quit; arrows never do ----

#[test]
fn update_quit_emits_cmd_quit() {
    let model = make_list_model(&["PROJ-1"]);
    let (_, cmds) = update(model, Msg::Quit);
    assert!(cmds.contains(&Cmd::Quit), "Quit msg must produce Cmd::Quit");
}

#[test]
fn update_down_never_emits_cmd_quit() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let (_, cmds) = update(model, Msg::Down);
    assert!(
        !cmds.contains(&Cmd::Quit),
        "Down must not produce Cmd::Quit"
    );
}

#[test]
fn update_up_never_emits_cmd_quit() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.selected = 1;
    let (_, cmds) = update(model, Msg::Up);
    assert!(!cmds.contains(&Cmd::Quit), "Up must not produce Cmd::Quit");
}

// ---- B1: AC3 — view renders to TestBackend buffer ----

#[test]
fn view_renders_header_columns_with_issues() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("KEY") || text.contains("CHAVE"),
        "header KEY missing"
    );
    assert!(
        text.contains("TYPE") || text.contains("TIPO"),
        "header TYPE missing"
    );
    assert!(text.contains("STATUS"), "header STATUS missing");
    assert!(
        text.contains("ASSIGNEE") || text.contains("RESPONSÁVEL"),
        "header ASSIGNEE missing"
    );
    assert!(
        text.contains("SUMMARY") || text.contains("RESUMO"),
        "header SUMMARY missing"
    );
}

#[test]
fn view_renders_each_issue_key() {
    let model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(text.contains("PROJ-1"), "PROJ-1 key must appear in buffer");
    assert!(text.contains("PROJ-2"), "PROJ-2 key must appear in buffer");
    assert!(text.contains("PROJ-3"), "PROJ-3 key must appear in buffer");
}

#[test]
fn view_empty_model_renders_no_issues_notice() {
    let model = make_list_model(&[]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("No issues.") || text.contains("Nenhuma issue encontrada."),
        "empty model must show 'No issues.' notice; got: {text}"
    );
}

#[test]
fn view_empty_model_still_renders_header_columns() {
    let model = make_list_model(&[]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("KEY") || text.contains("CHAVE"),
        "header KEY missing on empty model"
    );
    assert!(
        text.contains("STATUS"),
        "header STATUS missing on empty model"
    );
}

// ---- B1: AC4 — fetch error exits non-zero before raw mode (wiremock) ----

#[tokio::test]
async fn fetch_error_yields_nonzero_exit_before_raw_mode() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let conn = open_in_memory_store();
    let cache = crate::store::cache::TaskCache::new(&conn);
    let mut stderr = Vec::<u8>::new();

    let code = fetch_and_run(&instance, &cache, &mut stderr).await;

    assert_ne!(code, 0, "search error must yield non-zero exit");
    let err_output = String::from_utf8(stderr).expect("utf8");
    assert!(
        !err_output.is_empty(),
        "error message must be written to stderr"
    );
}

#[tokio::test]
async fn fetch_error_writes_error_message_to_stderr() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "bad@example.com".to_owned(),
        token: "wrong-token".to_owned(),
        account_id: None,
    };
    let conn = open_in_memory_store();
    let cache = crate::store::cache::TaskCache::new(&conn);
    let mut stderr = Vec::<u8>::new();

    let code = fetch_and_run(&instance, &cache, &mut stderr).await;

    assert_ne!(code, 0);
    let err_output = String::from_utf8(stderr).expect("utf8");
    assert!(
        err_output.contains("Error"),
        "stderr must contain 'Error'; got: {err_output:?}"
    );
}

// ---- B2: AC1 — Select on non-empty list sets screen=Detail and emits LoadDetail ----

#[test]
fn update_select_on_non_empty_list_emits_load_detail_and_sets_screen_detail() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::Detail);
    assert!(next.detail.is_none(), "detail must be None (loading state)");
    assert_eq!(next.detail_scroll, 0);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], Cmd::LoadDetail("PROJ-1".to_owned()));
}

#[test]
fn update_select_uses_selected_index_as_key() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    model.selected = 2;
    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::Detail);
    assert_eq!(cmds[0], Cmd::LoadDetail("PROJ-3".to_owned()));
}

#[test]
fn update_select_on_empty_list_is_noop_no_cmd() {
    let model = make_list_model(&[]);
    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::List, "screen must remain List");
    assert!(cmds.is_empty(), "empty list Select must emit no Cmd");
}

#[test]
fn update_select_on_detail_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::Detail, "screen stays Detail");
    assert!(cmds.is_empty());
}

// ---- B2: AC2 — Back and DetailLoaded transitions ----

#[test]
fn update_back_from_detail_sets_screen_list_preserves_selected() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.selected = 1;
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-2"));

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.selected, 1, "selection must be preserved after Back");
    assert!(next.detail.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_back_from_list_is_noop() {
    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert!(cmds.is_empty());
}

#[test]
fn update_detail_loaded_stores_issue_resets_scroll() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_scroll = 5;

    let issue = make_issue("PROJ-1");
    let (next, cmds) = update(model, Msg::DetailLoaded(Box::new(issue.clone())));

    assert_eq!(next.detail.as_ref().unwrap().key, "PROJ-1");
    assert_eq!(
        next.detail_scroll, 0,
        "scroll must reset to 0 on DetailLoaded"
    );
    assert!(cmds.is_empty());
}

// ---- B2: Down/Up on Detail scrolls ----

#[test]
fn update_down_on_detail_increments_scroll() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));
    model.detail_scroll = 2;

    let (next, cmds) = update(model, Msg::Down);

    assert_eq!(next.detail_scroll, 3);
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_detail_decrements_scroll_clamps_at_zero() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));
    model.detail_scroll = 0;

    let (next, cmds) = update(model, Msg::Up);

    assert_eq!(next.detail_scroll, 0, "scroll must not underflow below 0");
    assert!(cmds.is_empty());
}

// ---- B2: AC3 — Detail view renders to TestBackend ----

#[test]
fn view_detail_with_loaded_issue_shows_summary_status_and_description() {
    let mut model = make_list_model(&["PROJ-42"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-42"));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("PROJ-42"),
        "detail must show the issue key; got: {text}"
    );
    assert!(
        text.contains("Summary of the issue"),
        "detail must show the summary; got: {text}"
    );
    assert!(
        text.contains("In Progress"),
        "detail must show the status; got: {text}"
    );
    assert!(
        text.contains("Flattened description here."),
        "detail must show the flattened description; got: {text}"
    );
}

#[test]
fn view_detail_with_none_shows_loading_notice() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = None;

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Loading") || text.contains("…"),
        "None detail must show a loading/empty notice; got: {text}"
    );
}

#[test]
fn view_detail_shows_assignee() {
    let mut model = make_list_model(&["PROJ-7"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-7"));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Alice"),
        "detail must show the assignee display_name; got: {text}"
    );
}

// ---- issue 0021 A1: view_detail renders styled description runs ----

#[test]
fn view_detail_renders_bold_description_run_with_bold_modifier() {
    let mut model = make_list_model(&["PROJ-11"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_styled_description("PROJ-11"));

    let buf = render_to_buffer(&model, 120, 30);
    let style = style_at_text(&buf, "Bold text").expect("bold run must appear in buffer");

    assert!(
        style.add_modifier.contains(ratatui::style::Modifier::BOLD),
        "bold description run must carry Modifier::BOLD: {style:?}"
    );
}

#[test]
fn view_detail_renders_link_description_run_with_underlined_modifier() {
    let mut model = make_list_model(&["PROJ-12"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_styled_description("PROJ-12"));

    let buf = render_to_buffer(&model, 120, 30);
    let style = style_at_text(&buf, "a link").expect("link run must appear in buffer");

    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "link description run must carry Modifier::UNDERLINED: {style:?}"
    );
}

#[test]
fn view_detail_plain_description_run_carries_no_bold_or_underline() {
    let mut model = make_list_model(&["PROJ-13"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_styled_description("PROJ-13"));

    let buf = render_to_buffer(&model, 120, 30);
    let style = style_at_text(&buf, "plain then").expect("plain run must appear in buffer");

    assert!(
        !style.add_modifier.contains(ratatui::style::Modifier::BOLD),
        "plain run must not carry BOLD: {style:?}"
    );
    assert!(
        !style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "plain run must not carry UNDERLINED: {style:?}"
    );
}

// ---- i18n: view_detail field labels (issue 0014) ----

#[test]
fn view_detail_pt_br_translates_labels() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let mut model = make_list_model(&["PROJ-9"]);
    model.screen = Screen::Detail;
    model.detail = Some(crate::models::Issue {
        assignee: None,
        ..make_issue("PROJ-9")
    });

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(text.contains("Tipo"), "must show Tipo label: {text}");
    assert!(
        text.contains("Responsável"),
        "must show Responsável label: {text}"
    );
    assert!(
        text.contains("Descrição"),
        "must show Descrição label: {text}"
    );
    assert!(
        text.contains("Não atribuído"),
        "must show Não atribuído for unassigned issue: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_en_labels_unchanged() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(&["PROJ-42"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-42"));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(text.contains("Status:"), "en label must be Status: {text}");
    assert!(text.contains("Type:"), "en label must be Type: {text}");
    assert!(
        text.contains("Assignee:"),
        "en label must be Assignee: {text}"
    );
    assert!(
        text.contains("Description:"),
        "en label must be Description: {text}"
    );

    set_language("en");
}

// ---- B2: AC4 — cache hit / fetch error via load_issue (commands seam) ----

#[tokio::test]
async fn load_issue_cache_hit_serves_without_network_call() {
    use wiremock::MockServer;

    let server = MockServer::start().await;
    // No mocks mounted — any network call would be an unexpected request.

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let conn = open_in_memory_store();
    let issue_cache = crate::store::cache::IssueCache::new(&conn);

    let cached_issue = make_issue("PROJ-99");
    issue_cache.write(&instance.name, &cached_issue).unwrap();

    let mut sink: Vec<u8> = Vec::new();
    let result =
        crate::commands::load_issue("PROJ-99", &instance, &issue_cache, false, &mut sink).await;

    assert!(result.is_ok(), "cache hit must return Ok");
    assert_eq!(result.unwrap().key, "PROJ-99");

    let received = server.received_requests().await.unwrap();
    assert!(
        received.is_empty(),
        "cache hit must make zero network requests; got: {received:?}"
    );
}

#[tokio::test]
async fn load_issue_fetch_error_returns_err_leaving_ui_path_usable() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-404"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let conn = open_in_memory_store();
    let issue_cache = crate::store::cache::IssueCache::new(&conn);

    let mut sink: Vec<u8> = Vec::new();
    let result =
        crate::commands::load_issue("PROJ-404", &instance, &issue_cache, false, &mut sink).await;

    assert!(
        result.is_err(),
        "fetch error must return Err so the TUI can fall back gracefully"
    );
    // UI path remains usable: the TUI applies Msg::Back on Err, which is a pure transition
    // tested in update_back_from_detail_sets_screen_list_preserves_selected.
}

// ---- B3: AC1 — SubmitSearch emits Cmd::LoadList; edge cases are no-ops ----

#[test]
fn update_submit_search_with_non_empty_query_emits_load_list() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("project = PROJ".to_owned());

    let (_, cmds) = update(model, Msg::SubmitSearch);

    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], Cmd::LoadList("project = PROJ".to_owned()));
}

#[test]
fn update_submit_search_with_empty_query_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some(String::new());

    let (_, cmds) = update(model, Msg::SubmitSearch);

    assert!(cmds.is_empty(), "empty query SubmitSearch must emit no Cmd");
}

#[test]
fn update_submit_search_when_search_inactive_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (_, cmds) = update(model, Msg::SubmitSearch);

    assert!(
        cmds.is_empty(),
        "SubmitSearch when search==None must emit no Cmd"
    );
}

#[test]
fn update_submit_search_preserves_prior_rows_until_result() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.search = Some("project = PROJ".to_owned());

    let (next, _) = update(model, Msg::SubmitSearch);

    assert_eq!(
        next.rows.len(),
        2,
        "prior rows must be preserved while search is in-flight"
    );
}

// ---- B3: AC1 — typing transitions: OpenSearch, SearchInput, SearchBackspace, CancelSearch ----

#[test]
fn update_open_search_sets_search_to_empty_string() {
    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::OpenSearch);

    assert_eq!(
        next.search,
        Some(String::new()),
        "OpenSearch must set search=Some(\"\")"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_open_search_clears_error() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.error = Some("previous error".to_owned());

    let (next, _) = update(model, Msg::OpenSearch);

    assert!(
        next.error.is_none(),
        "OpenSearch must clear the error banner"
    );
}

#[test]
fn update_open_search_on_detail_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let (next, cmds) = update(model, Msg::OpenSearch);

    assert!(
        next.search.is_none(),
        "OpenSearch on Detail screen must not activate search"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_search_input_appends_character_to_query() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("pro".to_owned());

    let (next, cmds) = update(model, Msg::SearchInput('j'));

    assert_eq!(next.search.as_deref(), Some("proj"));
    assert!(cmds.is_empty());
}

#[test]
fn update_search_input_when_search_inactive_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::SearchInput('x'));

    assert!(next.search.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_search_backspace_pops_last_char() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("proj".to_owned());

    let (next, cmds) = update(model, Msg::SearchBackspace);

    assert_eq!(next.search.as_deref(), Some("pro"));
    assert!(cmds.is_empty());
}

#[test]
fn update_search_backspace_on_empty_query_stays_empty() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some(String::new());

    let (next, cmds) = update(model, Msg::SearchBackspace);

    assert_eq!(next.search.as_deref(), Some(""));
    assert!(cmds.is_empty());
}

#[test]
fn update_search_backspace_when_inactive_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::SearchBackspace);

    assert!(next.search.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_cancel_search_clears_search_state() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("project = X".to_owned());

    let (next, cmds) = update(model, Msg::CancelSearch);

    assert!(next.search.is_none(), "CancelSearch must set search=None");
    assert!(cmds.is_empty());
}

#[test]
fn update_cancel_search_preserves_rows_and_selection() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.selected = 1;
    model.search = Some("something".to_owned());

    let (next, _) = update(model, Msg::CancelSearch);

    assert_eq!(next.rows.len(), 2, "CancelSearch must preserve rows");
    assert_eq!(next.selected, 1, "CancelSearch must preserve selection");
}

// ---- B3: AC2 — LoadFailed sets error + preserves rows; ListLoaded replaces state ----

#[test]
fn update_load_failed_sets_error_banner_and_preserves_rows() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.search = Some("bad JQL".to_owned());

    let (next, cmds) = update(model, Msg::LoadFailed("invalid JQL syntax".to_owned()));

    assert_eq!(
        next.error.as_deref(),
        Some("invalid JQL syntax"),
        "LoadFailed must set the error banner"
    );
    assert!(next.search.is_none(), "LoadFailed must clear search state");
    assert_eq!(
        next.rows.len(),
        2,
        "LoadFailed must preserve the prior rows"
    );
    assert_eq!(next.rows[0].key, "PROJ-1", "row content must be unchanged");
    assert!(cmds.is_empty());
}

#[test]
fn update_list_loaded_replaces_rows_resets_selected_clears_search_and_error() {
    let mut model = make_list_model(&["OLD-1", "OLD-2"]);
    model.selected = 1;
    model.search = Some("project = NEW".to_owned());
    model.error = Some("old error".to_owned());
    model.next_page_token = Some("stale-token".to_owned());

    let new_rows = vec![make_row("NEW-1"), make_row("NEW-2"), make_row("NEW-3")];
    let (next, cmds) = update(
        model,
        Msg::ListLoaded(new_rows, Some("fresh-token".to_owned())),
    );

    assert_eq!(next.rows.len(), 3, "ListLoaded must replace rows");
    assert_eq!(next.rows[0].key, "NEW-1");
    assert_eq!(next.selected, 0, "ListLoaded must reset selected to 0");
    assert!(next.search.is_none(), "ListLoaded must clear search");
    assert!(
        next.error.is_none(),
        "ListLoaded must clear the error banner"
    );
    assert_eq!(
        next.next_page_token.as_deref(),
        Some("fresh-token"),
        "ListLoaded must set the paging cursor from the fresh result's token"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_list_loaded_with_no_token_clears_the_paging_cursor() {
    let mut model = make_list_model(&["OLD-1"]);
    model.next_page_token = Some("stale-token".to_owned());

    let (next, _) = update(model, Msg::ListLoaded(vec![make_row("NEW-1")], None));

    assert!(
        next.next_page_token.is_none(),
        "a fresh list with no token must clear any stale paging cursor"
    );
}

// ---- P3: AC2 — MoreLoaded appends rows, preserves selection, advances token ----

#[test]
fn update_more_loaded_appends_rows_preserves_selection_and_advances_token() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.selected = 1;
    model.next_page_token = Some("page-2-token".to_owned());

    let more_rows = vec![make_row("PROJ-3"), make_row("PROJ-4")];
    let (next, cmds) = update(
        model,
        Msg::MoreLoaded(more_rows, Some("page-3-token".to_owned())),
    );

    assert_eq!(
        next.rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        vec!["PROJ-1", "PROJ-2", "PROJ-3", "PROJ-4"],
        "MoreLoaded must append the new rows after the existing ones"
    );
    assert_eq!(
        next.selected, 1,
        "MoreLoaded must preserve the current selection"
    );
    assert_eq!(
        next.next_page_token.as_deref(),
        Some("page-3-token"),
        "MoreLoaded must advance the paging cursor to the new token"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_more_loaded_with_no_token_clears_the_paging_cursor() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, _) = update(model, Msg::MoreLoaded(vec![make_row("PROJ-2")], None));

    assert!(
        next.next_page_token.is_none(),
        "MoreLoaded on the last page must clear the paging cursor"
    );
}

// ---- P3: AC1 — LoadMore emits Cmd::LoadMore only on List with a pending token ----

#[test]
fn update_load_more_with_pending_token_on_list_emits_cmd_load_more() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.jql = "assignee = currentUser()".to_owned();
    model.next_page_token = Some("page-2-token".to_owned());

    let (_, cmds) = update(model, Msg::LoadMore);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::LoadMore(
            "assignee = currentUser()".to_owned(),
            "page-2-token".to_owned()
        ),
        "LoadMore must emit Cmd::LoadMore(jql, token) when a page is pending"
    );
}

#[test]
fn update_load_more_with_no_pending_token_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (_, cmds) = update(model, Msg::LoadMore);

    assert!(
        cmds.is_empty(),
        "LoadMore with no pending token must emit no Cmd"
    );
}

#[test]
fn update_load_more_on_detail_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.next_page_token = Some("page-2-token".to_owned());

    let (_, cmds) = update(model, Msg::LoadMore);

    assert!(
        cmds.is_empty(),
        "LoadMore on the Detail screen must emit no Cmd, even with a pending token"
    );
}

// ---- P3: AC3 — view_list shows the load-more affordance only when a token is pending ----

#[test]
fn view_list_shows_load_more_hint_when_token_pending() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.next_page_token = Some("page-2-token".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("more"),
        "footer must show a load-more affordance while a token is pending; got: {text}"
    );
}

#[test]
fn view_list_hides_load_more_hint_on_last_page() {
    let model = make_list_model(&["PROJ-1"]);

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("more"),
        "footer must not show the load-more affordance once the last page is loaded; got: {text}"
    );
}

// ---- B3: AC3 — view renders search bar and error banner to TestBackend ----

#[test]
fn view_with_search_active_shows_typed_query_in_buffer() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("project = X".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("project = X"),
        "buffer must show the typed query; got: {text}"
    );
    assert!(
        text.contains("JQL>"),
        "buffer must show the JQL> prompt; got: {text}"
    );
}

#[test]
fn view_with_error_and_rows_shows_banner_and_list() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2"]);
    model.error = Some("Invalid JQL query".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Invalid JQL query"),
        "buffer must show the error message; got: {text}"
    );
    assert!(
        text.contains("PROJ-1"),
        "buffer must still show list rows when error is set; got: {text}"
    );
}

#[test]
fn view_with_no_search_or_error_shows_normal_list() {
    let model = make_list_model(&["PROJ-5"]);

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("JQL>"),
        "normal list must not show the search prompt"
    );
    assert!(text.contains("PROJ-5"), "normal list must show issue keys");
}

// ---- B3: AC4 — run_search wiremock: valid JQL returns rows; 400 returns Err ----

#[tokio::test]
async fn run_search_with_valid_jql_returns_issue_rows() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload_with_key("SRCH-1")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = run_search(&instance, "project = SRCH").await;

    assert!(result.is_ok(), "valid JQL must return Ok; got: {result:?}");
    let result = result.unwrap();
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "SRCH-1");

    server.verify().await;
}

#[tokio::test]
async fn run_search_with_invalid_jql_returns_err() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"errorMessages":["The value 'BADJQL' does not exist for the field 'project'."]}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = run_search(&instance, "BADJQL").await;

    assert!(
        result.is_err(),
        "400 from server must yield Err (LoadFailed path)"
    );

    server.verify().await;
}

#[test]
fn update_load_failed_from_400_sets_error_and_preserves_rows() {
    let mut model = make_list_model(&["KEEP-1", "KEEP-2"]);
    model.search = Some("BADJQL".to_owned());

    let (next, cmds) = update(
        model,
        Msg::LoadFailed("search(BADJQL): 400 Bad Request".to_owned()),
    );

    assert!(
        next.error.is_some(),
        "LoadFailed must set error banner from 400 error"
    );
    assert_eq!(next.rows.len(), 2, "rows must be preserved after 400");
    assert_eq!(next.rows[0].key, "KEEP-1");
    assert!(next.search.is_none());
    assert!(cmds.is_empty());
}

// ---- issue 0020: browse TUI chrome i18n parity ----

#[test]
fn view_list_pt_br_translates_footer_error_banner_and_more_hint() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let mut model = make_list_model(&["PROJ-1"]);
    model.next_page_token = Some("page-2-token".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("↑/↓ navegar") && text.contains("buscar") && text.contains("q sair"),
        "must show the translated normal-list footer; got: {text}"
    );
    assert!(
        text.contains("n mais"),
        "must show 'n mais' hint; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_list_pt_br_translates_search_footer_and_error_banner() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("project = X".to_owned());
    model.error = Some("bad JQL".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Enter enviar")
            && text.contains("Esc cancelar")
            && text.contains("Backspace apagar"),
        "must show the translated search footer; got: {text}"
    );
    assert!(
        text.contains("Erro: bad JQL"),
        "must show the translated error-banner prefix; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_list_en_chrome_is_identity() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(&["PROJ-1"]);
    model.next_page_token = Some("page-2-token".to_owned());

    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("↑/↓ navigate  /  search  Enter select  Esc/b back  q quit"),
        "en footer must remain byte-identical to the pre-change render; got: {text}"
    );
    assert!(
        text.contains("n more"),
        "en 'n more' hint must be unchanged; got: {text}"
    );

    let mut search_model = make_list_model(&["PROJ-1"]);
    search_model.search = Some("project = X".to_owned());
    search_model.error = Some("bad JQL".to_owned());

    let search_buf = render_to_buffer(&search_model, 120, 20);
    let search_text = buffer_text(&search_buf);

    assert!(
        search_text.contains("Enter submit  Esc cancel  Backspace delete"),
        "en search footer must remain byte-identical; got: {search_text}"
    );
    assert!(
        search_text.contains("Error: bad JQL"),
        "en error banner must remain byte-identical; got: {search_text}"
    );

    set_language("en");
}

// ---- B4: AC1 — update(OpenLink) emits Cmd::OpenUrl; empty list is no-op ----

#[test]
fn update_open_link_on_non_empty_list_emits_open_url_with_browse_url() {
    let mut model = make_list_model(&["PROJ-7", "PROJ-8"]);
    model.base_url = "https://acme.atlassian.net/".to_owned();
    model.selected = 0;

    let (_, cmds) = update(model, Msg::OpenLink);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::OpenUrl("https://acme.atlassian.net/browse/PROJ-7".to_owned()),
        "OpenLink must emit Cmd::OpenUrl with the trimmed base_url and selected key"
    );
}

#[test]
fn update_open_link_uses_selected_index() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    model.base_url = "https://acme.atlassian.net".to_owned();
    model.selected = 2;

    let (_, cmds) = update(model, Msg::OpenLink);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::OpenUrl("https://acme.atlassian.net/browse/PROJ-3".to_owned()),
        "OpenLink must use model.selected as the index into rows"
    );
}

#[test]
fn update_open_link_trims_trailing_slash_from_base_url() {
    let mut model = make_list_model(&["KEY-1"]);
    model.base_url = "https://acme.atlassian.net///".to_owned();

    let (_, cmds) = update(model, Msg::OpenLink);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::OpenUrl("https://acme.atlassian.net/browse/KEY-1".to_owned()),
        "trailing slashes in base_url must be trimmed before building the URL"
    );
}

#[test]
fn update_open_link_on_empty_list_is_noop() {
    let model = make_list_model(&[]);

    let (_, cmds) = update(model, Msg::OpenLink);

    assert!(
        cmds.is_empty(),
        "OpenLink on an empty list must emit no Cmd"
    );
}

// ---- B4: AC2 — update(CopyKey) emits Cmd::CopyToClipboard; empty list is no-op ----

#[test]
fn update_copy_key_on_non_empty_list_emits_copy_to_clipboard_with_selected_key() {
    let model = make_list_model(&["PROJ-42", "PROJ-43"]);

    let (_, cmds) = update(model, Msg::CopyKey);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::CopyToClipboard("PROJ-42".to_owned()),
        "CopyKey must emit Cmd::CopyToClipboard with the selected issue key"
    );
}

#[test]
fn update_copy_key_uses_selected_index() {
    let mut model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    model.selected = 1;

    let (_, cmds) = update(model, Msg::CopyKey);

    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        Cmd::CopyToClipboard("PROJ-2".to_owned()),
        "CopyKey must use model.selected to pick the correct key"
    );
}

#[test]
fn update_copy_key_on_empty_list_is_noop() {
    let model = make_list_model(&[]);

    let (_, cmds) = update(model, Msg::CopyKey);

    assert!(cmds.is_empty(), "CopyKey on an empty list must emit no Cmd");
}

// ---- B4: AC3 — issue_browse_url is the single source; render and agent_json agree ----

#[test]
fn issue_browse_url_builds_correct_url() {
    let url = crate::render::issue_browse_url("https://acme.atlassian.net", "PROJ-99");
    assert_eq!(url, "https://acme.atlassian.net/browse/PROJ-99");
}

#[test]
fn issue_browse_url_trims_trailing_slash() {
    let url = crate::render::issue_browse_url("https://acme.atlassian.net/", "PROJ-1");
    assert_eq!(
        url, "https://acme.atlassian.net/browse/PROJ-1",
        "trailing slash must be trimmed from base_url"
    );
}

#[test]
fn render_issue_human_url_equals_issue_browse_url() {
    let issue = make_issue("PROJ-55");
    let base = "https://acme.atlassian.net";
    let expected_url = crate::render::issue_browse_url(base, "PROJ-55");

    let mut out = Vec::new();
    crate::render::render_issue_human(&issue, "work", base, false, &mut out);
    let text = std::str::from_utf8(&out).unwrap();

    assert!(
        text.contains(&expected_url),
        "render_issue_human must embed the URL produced by issue_browse_url; expected {expected_url:?} in:\n{text}"
    );
}

#[test]
fn agent_json_url_field_equals_issue_browse_url() {
    let issue = make_issue("PROJ-77");
    let base = "https://acme.atlassian.net";
    let expected_url = crate::render::issue_browse_url(base, "PROJ-77");

    let obj = crate::agent_json::issue_object(&issue, "work", base, false);
    let json_url = obj["url"].as_str().unwrap();

    assert_eq!(
        json_url, expected_url,
        "agent_json url field must equal issue_browse_url output"
    );
}

#[test]
fn render_and_agent_json_produce_same_url_for_same_input() {
    let issue = make_issue("PROJ-123");
    let base = "https://acme.atlassian.net/";

    let expected_url = crate::render::issue_browse_url(base, "PROJ-123");

    let mut out = Vec::new();
    crate::render::render_issue_human(&issue, "work", base, false, &mut out);
    let render_text = std::str::from_utf8(&out).unwrap();

    let obj = crate::agent_json::issue_object(&issue, "work", base, false);
    let json_url = obj["url"].as_str().unwrap();

    assert!(
        render_text.contains(&expected_url),
        "render_issue_human must contain the canonical URL"
    );
    assert_eq!(
        json_url, expected_url,
        "agent_json url must equal the canonical URL"
    );
}
