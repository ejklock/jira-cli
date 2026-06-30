use super::*;

use crate::cli::{browse_tty_action, BrowseAction};
use crate::models::IssueRow;
use ratatui::{backend::TestBackend, Terminal};

// ---- Helpers ----

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
        description: Some(r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"Flattened description here."}]}]}"#.to_owned()),
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
