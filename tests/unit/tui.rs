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
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, false, &mut stderr).await;

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
    let instance = make_test_instance();
    let mut stderr = Vec::<u8>::new();

    let code = browse(&instance, false, &mut stderr).await;

    assert_ne!(code, 0);
}

// ---- B1: AC1 — update Down/Up movement and clamping ----

#[test]
fn update_down_increments_selected() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2", "PROJ-3"]),
        selected: 0,
    };
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(next.selected, 1);
    assert!(cmds.is_empty());
}

#[test]
fn update_down_clamps_at_last_row() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2"]),
        selected: 1,
    };
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(next.selected, 1, "Down at last row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_up_decrements_selected() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2", "PROJ-3"]),
        selected: 2,
    };
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(next.selected, 1);
    assert!(cmds.is_empty());
}

#[test]
fn update_up_clamps_at_zero() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2"]),
        selected: 0,
    };
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(next.selected, 0, "Up at first row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_down_on_empty_rows_is_noop() {
    let model = Model {
        rows: vec![],
        selected: 0,
    };
    let (next, cmds) = update(model, Msg::Down);
    assert_eq!(
        next.selected, 0,
        "Down on empty list must not panic or change selected"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_empty_rows_is_noop() {
    let model = Model {
        rows: vec![],
        selected: 0,
    };
    let (next, cmds) = update(model, Msg::Up);
    assert_eq!(
        next.selected, 0,
        "Up on empty list must not panic or change selected"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_preserves_rows_on_navigation() {
    let rows = make_rows(&["PROJ-1", "PROJ-2"]);
    let model = Model {
        rows: rows.clone(),
        selected: 0,
    };
    let (next, _) = update(model, Msg::Down);
    assert_eq!(next.rows.len(), 2, "rows must be preserved through update");
    assert_eq!(next.rows[0].key, "PROJ-1");
}

// ---- B1: AC2 — update Quit emits Cmd::Quit; arrows never do ----

#[test]
fn update_quit_emits_cmd_quit() {
    let model = Model {
        rows: make_rows(&["PROJ-1"]),
        selected: 0,
    };
    let (_, cmds) = update(model, Msg::Quit);
    assert!(cmds.contains(&Cmd::Quit), "Quit msg must produce Cmd::Quit");
}

#[test]
fn update_down_never_emits_cmd_quit() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2"]),
        selected: 0,
    };
    let (_, cmds) = update(model, Msg::Down);
    assert!(
        !cmds.contains(&Cmd::Quit),
        "Down must not produce Cmd::Quit"
    );
}

#[test]
fn update_up_never_emits_cmd_quit() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2"]),
        selected: 1,
    };
    let (_, cmds) = update(model, Msg::Up);
    assert!(!cmds.contains(&Cmd::Quit), "Up must not produce Cmd::Quit");
}

// ---- B1: AC3 — view renders to TestBackend buffer ----

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

#[test]
fn view_renders_header_columns_with_issues() {
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2"]),
        selected: 0,
    };
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
    let model = Model {
        rows: make_rows(&["PROJ-1", "PROJ-2", "PROJ-3"]),
        selected: 0,
    };
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(text.contains("PROJ-1"), "PROJ-1 key must appear in buffer");
    assert!(text.contains("PROJ-2"), "PROJ-2 key must appear in buffer");
    assert!(text.contains("PROJ-3"), "PROJ-3 key must appear in buffer");
}

#[test]
fn view_empty_model_renders_no_issues_notice() {
    let model = Model {
        rows: vec![],
        selected: 0,
    };
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("No issues.") || text.contains("Nenhuma issue encontrada."),
        "empty model must show 'No issues.' notice; got: {text}"
    );
}

#[test]
fn view_empty_model_still_renders_header_columns() {
    let model = Model {
        rows: vec![],
        selected: 0,
    };
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

    // Return 500 for any search request — simulates a server-side error.
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
    let mut stderr = Vec::<u8>::new();

    // Call fetch_and_run directly — it fetches before touching the terminal.
    let code = fetch_and_run(&instance, &mut stderr).await;

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
    let mut stderr = Vec::<u8>::new();

    let code = fetch_and_run(&instance, &mut stderr).await;

    assert_ne!(code, 0);
    let err_output = String::from_utf8(stderr).expect("utf8");
    assert!(
        err_output.contains("Error"),
        "stderr must contain 'Error'; got: {err_output:?}"
    );
}
