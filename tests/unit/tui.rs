use super::*;

use super::model::{entry_cmds, footer_mode, FooterMode, StatusKind, StatusMsg};
use super::shell::{
    map_key_in_normal_mode, map_key_in_search_mode, map_mouse_to_msg, read_snapshot,
    resolve_mouse_msg, MouseIntent,
};
use super::view;
use crate::cli::{browse_tty_action, BrowseAction};
use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::IssueRow;
use crate::store::cache::{instances_key, TaskListCache};
use crate::test_support::*;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

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
        duedate: None,
        project: None,
    }
}

fn make_rows(keys: &[&str]) -> Vec<IssueRow> {
    keys.iter().map(|k| make_row(k)).collect()
}

fn mouse_event(kind: MouseEventKind) -> MouseEvent {
    MouseEvent {
        kind,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }
}

fn mouse_click_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// Every `MouseEventKind` variant (BDR 0009 S6's property scope): every
/// button's press/release/drag, movement, and every wheel direction.
fn all_mouse_event_kinds() -> Vec<MouseEventKind> {
    let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
    let mut kinds: Vec<MouseEventKind> = buttons
        .iter()
        .flat_map(|&button| {
            [
                MouseEventKind::Down(button),
                MouseEventKind::Up(button),
                MouseEventKind::Drag(button),
            ]
        })
        .collect();
    kinds.extend([
        MouseEventKind::Moved,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ]);
    kinds
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
        detail_links: vec![],
        detail_focused_link: None,
        identities: vec![],
        status: None,
        revalidating: false,
    }
}

fn make_issue(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        summary: "Summary of the issue".to_owned(),
        status: "In Progress".to_owned(),
        assignee: Some(assignee("Alice", None)),
        description: Some(plain_paragraph("Flattened description here.")),
        ..issue(key)
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

/// A fully-migrated on-disk store (unlike `open_in_memory_store`'s minimal
/// hand-rolled schema): needed by the `task_list_cache` snapshot tests (BDR
/// 0008 S2/S3/S7), which imitates `tests/unit/store/cache.rs`'s `make_store`.
fn open_temp_store() -> (tempfile::TempDir, crate::store::Store) {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = crate::config::Config {
        db_path,
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    (dir, store)
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

fn make_issue_with_comments(
    key: &str,
    comments: Vec<crate::models::IssueComment>,
) -> crate::models::Issue {
    crate::models::Issue {
        comments,
        ..make_issue(key)
    }
}

fn make_issue_with_two_links(key: &str) -> crate::models::Issue {
    crate::models::Issue {
        description: Some(doc(vec![paragraph(vec![
            marked_text("first link", vec![link_mark("https://example.com/first")]),
            text(" and "),
            marked_text("second link", vec![link_mark("https://example.com/second")]),
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

// issue 0031 (D2): the list Table (with KEY/TYPE/STATUS/ASSIGNEE/SUMMARY
// column headers) was replaced by per-issue cards with no header row; card
// content coverage lives in tests/unit/tui_render.rs (BDR 0007 S2-S4).
#[test]
fn view_renders_each_issue_as_a_card_showing_key_and_status() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(text.contains("PROJ-1"), "PROJ-1 key must appear in buffer");
    assert!(text.contains("PROJ-2"), "PROJ-2 key must appear in buffer");
    assert!(
        text.contains("Open"),
        "card meta line must show the row's status; got: {text}"
    );
}

#[test]
fn view_renders_each_issue_key() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(text.contains("PROJ-1"), "PROJ-1 key must appear in buffer");
    assert!(text.contains("PROJ-2"), "PROJ-2 key must appear in buffer");
    assert!(text.contains("PROJ-3"), "PROJ-3 key must appear in buffer");
}

#[test]
fn view_empty_model_renders_no_issues_notice() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let model = make_list_model(&[]);
    let buf = render_to_buffer(&model, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("No issues.") || text.contains("Nenhuma issue encontrada."),
        "empty model must show 'No issues.' notice; got: {text}"
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

// ---- e3-swr-first-paint-browse-entry (ADR 0016 / BDR 0008) ----

// ---- S3: cold fetch_and_run success writes the mine-scope snapshot;
// existing failure-path tests above stay untouched (no-drift guard) ----

#[tokio::test]
async fn fetch_and_run_cold_success_writes_the_mine_scope_snapshot() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload_with_key("SNAP-1")),
        )
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let mut stderr = Vec::<u8>::new();

    // fetch_and_run opens the TUI on success; bounding it keeps the test from
    // hanging in an environment where a real terminal happens to be attached
    // (the snapshot write under test always runs before that point).
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        fetch_and_run(&instance, &cache, &mut stderr),
    )
    .await;

    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    let stored = list_cache
        .read("mine", &key, 3600)
        .expect("read must not error");
    let stored = stored.expect("cold success must write the (\"mine\", instances_key) row");
    assert!(
        stored.contains("SNAP-1"),
        "the stored snapshot must contain the fetched issue; got: {stored}"
    );
}

// ---- read_snapshot: warm row, corrupt json, and over-max-age rows ----

#[test]
fn read_snapshot_returns_rows_for_a_warm_row() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();
    let rows = make_rows(&["PROJ-1", "PROJ-2"]);
    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    list_cache
        .write("mine", &key, &serde_json::to_string(&rows).unwrap())
        .unwrap();

    let result = read_snapshot(&cache, &instance);

    assert_eq!(
        result.map(|r| r.into_iter().map(|row| row.key).collect::<Vec<_>>()),
        Some(vec!["PROJ-1".to_owned(), "PROJ-2".to_owned()])
    );
}

#[test]
fn read_snapshot_returns_none_for_corrupt_json() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();
    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    list_cache.write("mine", &key, "not valid json").unwrap();

    assert!(
        read_snapshot(&cache, &instance).is_none(),
        "undeserializable JSON must be a cold entry, never an error"
    );
}

#[test]
fn read_snapshot_returns_none_for_a_row_older_than_max_age() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();
    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    let rows = make_rows(&["PROJ-1"]);
    let stale_ts = crate::store::now_epoch_secs() - (8 * 24 * 60 * 60);
    list_cache
        .write_with_fetched_at(
            "mine",
            &key,
            &serde_json::to_string(&rows).unwrap(),
            stale_ts,
        )
        .unwrap();

    assert!(
        read_snapshot(&cache, &instance).is_none(),
        "a row older than the 7-day max-age must be a cold entry"
    );
}

#[test]
fn read_snapshot_returns_none_when_no_row_exists() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();

    assert!(read_snapshot(&cache, &instance).is_none());
}

// ---- S1: entry_cmds — the pure warm/cold seam ----

#[test]
fn entry_cmds_warm_yields_exactly_one_revalidate_list() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.revalidating = true;

    assert_eq!(entry_cmds(&model), vec![Cmd::RevalidateList]);
}

#[test]
fn entry_cmds_cold_yields_no_cmds() {
    let model = make_list_model(&["PROJ-1"]);
    assert!(!model.revalidating);

    assert!(entry_cmds(&model).is_empty());
}

// ---- S2: RevalidationLoaded swaps rows, clamps selection, restores token ----

#[test]
fn update_revalidation_loaded_swaps_rows_clamps_selection_and_clears_flag() {
    let mut model = make_list_model(&["OLD-1", "OLD-2", "OLD-3"]);
    model.revalidating = true;
    model.selected = 2;

    let new_rows = vec![make_row("NEW-1"), make_row("NEW-2")];
    let (next, cmds) = update(
        model,
        Msg::RevalidationLoaded(new_rows, Some("fresh-token".to_owned())),
    );

    assert_eq!(
        next.rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        vec!["NEW-1".to_owned(), "NEW-2".to_owned()],
        "RevalidationLoaded must swap in the fresh rows"
    );
    assert_eq!(
        next.selected, 1,
        "selection must clamp to the new last index, not reset to 0"
    );
    assert_eq!(next.next_page_token.as_deref(), Some("fresh-token"));
    assert!(
        !next.revalidating,
        "revalidating must clear once the swap applies"
    );
    assert!(cmds.is_empty());
}

// ---- S4: a late RevalidationLoaded never clobbers a newer search ----

#[test]
fn update_revalidation_loaded_when_not_revalidating_is_a_pure_noop() {
    let model = make_list_model(&["SEARCH-1"]);
    assert!(!model.revalidating);

    let (next, cmds) = update(
        model,
        Msg::RevalidationLoaded(vec![make_row("STALE-1")], Some("stale-token".to_owned())),
    );

    assert_eq!(
        next.rows[0].key, "SEARCH-1",
        "a late revalidation result must not clobber the current rows"
    );
    assert!(next.next_page_token.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_submit_search_clears_revalidating_so_a_later_revalidation_is_ignored() {
    let mut model = make_list_model(&["OLD-1"]);
    model.revalidating = true;
    model.search = Some("project = NEW".to_owned());

    let (after_search, cmds) = update(model, Msg::SubmitSearch);
    assert!(
        !after_search.revalidating,
        "submitting a search must clear revalidating"
    );
    assert_eq!(cmds, vec![Cmd::LoadList("project = NEW".to_owned())]);

    let (after_search_result, _) = update(
        after_search,
        Msg::ListLoaded(vec![make_row("FRESH-1")], None),
    );
    assert_eq!(after_search_result.rows[0].key, "FRESH-1");

    let (after_stale_revalidation, _) = update(
        after_search_result,
        Msg::RevalidationLoaded(vec![make_row("STALE-1")], None),
    );

    assert_eq!(
        after_stale_revalidation.rows[0].key, "FRESH-1",
        "a revalidation result arriving after a newer search must be ignored"
    );
}

// ---- S5: RevalidationFailed keeps the painted rows and surfaces D4 status ----

#[test]
fn update_revalidation_failed_keeps_rows_clears_flag_and_sets_error_status() {
    let mut model = make_list_model(&["KEEP-1", "KEEP-2"]);
    model.revalidating = true;

    let (next, cmds) = update(model, Msg::RevalidationFailed("network down".to_owned()));

    assert_eq!(
        next.rows.len(),
        2,
        "the painted rows must be kept on a revalidation failure"
    );
    assert!(!next.revalidating, "revalidating must clear on failure");
    let status = next
        .status
        .expect("RevalidationFailed must set a status message");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, "network down");
    assert!(cmds.is_empty());
}

#[test]
fn update_revalidation_failed_when_not_revalidating_is_a_pure_noop() {
    let model = make_list_model(&["KEEP-1"]);

    let (next, cmds) = update(
        model,
        Msg::RevalidationFailed("should be ignored".to_owned()),
    );

    assert!(
        next.status.is_none(),
        "a RevalidationFailed with no revalidation in flight must not set a status"
    );
    assert!(cmds.is_empty());
}

// ---- S6: single-flight — load-more is dropped while revalidating ----

#[test]
fn update_load_more_while_revalidating_drops_and_leaves_model_unchanged() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.revalidating = true;
    model.next_page_token = Some("page-2-token".to_owned());

    let (next, cmds) = update(model, Msg::LoadMore);

    assert!(
        cmds.is_empty(),
        "load-more while revalidating must emit no Cmd"
    );
    assert!(next.revalidating, "the revalidating flag must be unchanged");
    assert_eq!(next.next_page_token.as_deref(), Some("page-2-token"));
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

// ---- B1 mouse foundations / BDR 0009 S3, S4 — CardClicked mirrors Select ----

#[test]
fn update_card_clicked_in_range_sets_selected_and_emits_load_detail() {
    let model = make_list_model(&["PROJ-1", "PROJ-2", "PROJ-3"]);
    let (next, cmds) = update(model, Msg::CardClicked(2));

    assert_eq!(next.selected, 2);
    assert_eq!(next.screen, Screen::Detail);
    assert!(next.detail.is_none(), "detail must be None (loading state)");
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], Cmd::LoadDetail("PROJ-3".to_owned()));
}

#[test]
fn update_card_clicked_out_of_range_is_noop_no_panic() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let (next, cmds) = update(model, Msg::CardClicked(5));

    assert_eq!(
        next.screen,
        Screen::List,
        "out-of-range click must not open detail"
    );
    assert_eq!(next.selected, 0, "selection must be unchanged");
    assert!(cmds.is_empty());
}

#[test]
fn update_card_clicked_on_empty_list_is_noop_no_panic() {
    let model = make_list_model(&[]);
    let (next, cmds) = update(model, Msg::CardClicked(0));

    assert_eq!(next.screen, Screen::List);
    assert!(cmds.is_empty());
}

#[test]
fn update_card_clicked_on_detail_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    let (next, cmds) = update(model, Msg::CardClicked(0));

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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
        text.contains("Description"),
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

// ---- issue 0022 A2: DetailLoaded populates detail_links + focus; Back clears ----

#[test]
fn update_detail_loaded_populates_detail_links_and_focuses_first() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let issue = make_issue_with_two_links("PROJ-1");
    let (next, cmds) = update(model, Msg::DetailLoaded(Box::new(issue)));

    assert_eq!(
        next.detail_links,
        vec![
            "https://example.com/first".to_owned(),
            "https://example.com/second".to_owned(),
        ],
        "detail_links must hold the description's inline hrefs in document order"
    );
    assert_eq!(
        next.detail_focused_link,
        Some(0),
        "the first link must be focused by default"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_detail_loaded_with_no_links_leaves_focus_none() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let issue = make_issue("PROJ-1");
    let (next, _) = update(model, Msg::DetailLoaded(Box::new(issue)));

    assert!(
        next.detail_links.is_empty(),
        "a description with no links must yield empty detail_links"
    );
    assert_eq!(
        next.detail_focused_link, None,
        "a description with no links must leave detail_focused_link None"
    );
}

#[test]
fn update_back_clears_detail_links_and_focus() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_two_links("PROJ-1"));
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(1);

    let (next, cmds) = update(model, Msg::Back);

    assert!(next.detail_links.is_empty(), "Back must clear detail_links");
    assert_eq!(
        next.detail_focused_link, None,
        "Back must clear detail_focused_link"
    );
    assert!(cmds.is_empty());
}

// ---- issue 0022 A2: FocusNextLink advances (wrapping); no-op with no links / on List ----

#[test]
fn update_focus_next_link_advances_index() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(0);

    let (next, cmds) = update(model, Msg::FocusNextLink);

    assert_eq!(next.detail_focused_link, Some(1));
    assert!(cmds.is_empty());
}

#[test]
fn update_focus_next_link_wraps_to_zero() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(1);

    let (next, cmds) = update(model, Msg::FocusNextLink);

    assert_eq!(
        next.detail_focused_link,
        Some(0),
        "focus must wrap back to the first link"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_focus_next_link_with_no_links_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let (next, cmds) = update(model, Msg::FocusNextLink);

    assert_eq!(next.detail_focused_link, None);
    assert!(cmds.is_empty());
}

#[test]
fn update_focus_next_link_on_list_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.detail_links = vec!["https://example.com/first".to_owned()];
    model.detail_focused_link = Some(0);

    let (next, cmds) = update(model, Msg::FocusNextLink);

    assert_eq!(
        next.detail_focused_link,
        Some(0),
        "FocusNextLink on the List screen must not change focus"
    );
    assert!(cmds.is_empty());
}

// ---- issue 0022 A2: Select on Detail opens the focused link (List unchanged) ----

#[test]
fn update_select_on_detail_with_focused_link_emits_open_url() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(1);

    let (_, cmds) = update(model, Msg::Select);

    assert_eq!(
        cmds,
        vec![Cmd::OpenUrl("https://example.com/second".to_owned())],
        "Select on Detail must open the focused link's href"
    );
}

#[test]
fn update_select_on_detail_with_no_focused_link_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let (_, cmds) = update(model, Msg::Select);

    assert!(
        cmds.is_empty(),
        "Select on Detail with no focused link must emit no Cmd"
    );
}

// ---- issue 0022 A2: view_detail highlights the focused inline link ----

#[test]
fn view_detail_highlights_focused_link_with_reversed_modifier() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-20"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_two_links("PROJ-20"));
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(0);

    let buf = render_to_buffer(&model, 120, 30);
    let style = style_at_text(&buf, "first link").expect("focused link run must appear in buffer");

    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "focused link must carry Modifier::REVERSED: {style:?}"
    );
    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "focused link must still carry Modifier::UNDERLINED: {style:?}"
    );
}

#[test]
fn view_detail_non_focused_link_has_no_reversed_modifier() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-21"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_two_links("PROJ-21"));
    model.detail_links = vec![
        "https://example.com/first".to_owned(),
        "https://example.com/second".to_owned(),
    ];
    model.detail_focused_link = Some(0);

    let buf = render_to_buffer(&model, 120, 30);
    let style =
        style_at_text(&buf, "second link").expect("non-focused link run must appear in buffer");

    assert!(
        !style
            .add_modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "non-focused link must not carry Modifier::REVERSED: {style:?}"
    );
    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "non-focused link must still carry Modifier::UNDERLINED: {style:?}"
    );
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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
    let _lock = LANG_MUTEX.lock().unwrap();
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

// ---- issue 0024 A4: comments rendered in the browse TUI detail ----

#[test]
fn view_detail_renders_comments_header_authors_and_bodies() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-30"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_comments(
        "PROJ-30",
        vec![
            comment(
                None,
                Some("Alice"),
                &doc(vec![paragraph(vec![text("First comment body.")])]),
                Some("2026-01-01"),
                None,
            ),
            comment(
                None,
                None,
                &doc(vec![paragraph(vec![text("Second comment body.")])]),
                None,
                None,
            ),
        ],
    ));

    let buf = render_to_buffer(&model, 120, 40);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Comments (2)") || text.contains("Comentários (2)"),
        "detail must show the Comments (N) panel title; got: {text}"
    );
    assert!(text.contains("[Alice] 2026-01-01"), "got: {text}");
    assert!(
        text.contains("First comment body."),
        "first comment body missing; got: {text}"
    );
    assert!(
        text.contains("[Unknown] ") || text.contains("[Desconhecido] "),
        "author-less comment must fall back to Unknown; got: {text}"
    );
    assert!(
        text.contains("Second comment body."),
        "second comment body missing; got: {text}"
    );
}

#[test]
fn view_detail_renders_bold_comment_body_run_with_bold_modifier() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-31"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_comments(
        "PROJ-31",
        vec![comment(
            None,
            Some("Bob"),
            &doc(vec![paragraph(vec![marked_text(
                "Bold comment",
                vec![mark("strong")],
            )])]),
            Some("2026-02-02"),
            None,
        )],
    ));

    let buf = render_to_buffer(&model, 120, 40);
    let style = style_at_text(&buf, "Bold comment").expect("bold comment run must appear");

    assert!(
        style.add_modifier.contains(ratatui::style::Modifier::BOLD),
        "bold comment run must carry Modifier::BOLD: {style:?}"
    );
}

#[test]
fn view_detail_renders_link_comment_body_run_with_underlined_modifier() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-32"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_comments(
        "PROJ-32",
        vec![comment(
            None,
            Some("Bob"),
            &doc(vec![paragraph(vec![marked_text(
                "linked comment text",
                vec![link_mark("https://example.com/comment")],
            )])]),
            Some("2026-02-03"),
            None,
        )],
    ));

    let buf = render_to_buffer(&model, 120, 40);
    let style = style_at_text(&buf, "linked comment text").expect("link comment run must appear");

    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "link comment run must carry Modifier::UNDERLINED: {style:?}"
    );
}

#[test]
fn view_detail_with_no_comments_renders_no_comments_header() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-33"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_comments("PROJ-33", vec![]));

    let buf = render_to_buffer(&model, 120, 40);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("Comments:") && !text.contains("Comentários:"),
        "empty comments must render no Comments header; got: {text}"
    );
}

// ---- issue 0026 A3b: view_detail Due line (ADR 0013) ----

fn make_issue_with_duedate(key: &str, duedate: Option<String>) -> crate::models::Issue {
    crate::models::Issue {
        duedate,
        ..make_issue(key)
    }
}

#[test]
fn view_detail_shows_due_line_after_assignee_when_duedate_parses() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let mut model = make_list_model(&["PROJ-50"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_duedate(
        "PROJ-50",
        Some(duedate_offset_from_today(3)),
    ));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Due: in 3 days"),
        "detail must show Due: in 3 days; got: {text}"
    );
    let assignee_pos = text
        .find("Assignee:")
        .expect("Assignee line must be present");
    let due_pos = text.find("Due:").expect("Due line must be present");
    assert!(
        due_pos > assignee_pos,
        "Due line must come after the Assignee line; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_pt_br_shows_translated_due_line() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let mut model = make_list_model(&["PROJ-51"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_duedate(
        "PROJ-51",
        Some(duedate_offset_from_today(3)),
    ));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Prazo: em 3 dias"),
        "detail must show Prazo: em 3 dias; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_omits_due_line_when_duedate_is_none() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let mut model = make_list_model(&["PROJ-52"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_duedate("PROJ-52", None));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("Due:") && !text.contains("Prazo:"),
        "no duedate must omit the Due line; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_omits_due_line_when_duedate_unparseable() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let mut model = make_list_model(&["PROJ-53"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_duedate(
        "PROJ-53",
        Some("not-a-date".to_owned()),
    ));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        !text.contains("Due:"),
        "unparseable duedate must omit the Due line; got: {text}"
    );

    set_language("en");
}

#[test]
fn map_key_in_normal_mode_j_is_down() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('j'), KeyModifiers::NONE),
        Some(Msg::Down)
    ));
}

#[test]
fn map_key_in_normal_mode_k_is_up() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('k'), KeyModifiers::NONE),
        Some(Msg::Up)
    ));
}

// ---- B1 mouse foundations / BDR 0009 S1, S2, S7 — wheel mapper ----

#[test]
fn map_mouse_to_msg_scroll_up_is_nav_up_in_normal_mode() {
    let mouse = mouse_event(MouseEventKind::ScrollUp);
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Nav(Msg::Up))
    ));
}

#[test]
fn map_mouse_to_msg_scroll_down_is_nav_down_in_normal_mode() {
    let mouse = mouse_event(MouseEventKind::ScrollDown);
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Nav(Msg::Down))
    ));
}

#[test]
fn map_mouse_to_msg_left_down_is_click_intent_with_coordinates() {
    let mouse = mouse_click_at(12, 7);
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Click { x: 12, y: 7 })
    ));
}

#[test]
fn map_mouse_to_msg_search_active_swallows_scroll() {
    let mouse = mouse_event(MouseEventKind::ScrollUp);
    assert!(map_mouse_to_msg(mouse, true).is_none());
}

#[test]
fn map_mouse_to_msg_search_active_swallows_click() {
    let mouse = mouse_click_at(5, 5);
    assert!(map_mouse_to_msg(mouse, true).is_none());
}

#[test]
fn map_mouse_to_msg_ignores_drag_and_non_left_buttons() {
    for kind in [
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Moved,
    ] {
        assert!(
            map_mouse_to_msg(mouse_event(kind), false).is_none(),
            "{kind:?} must not map to any intent"
        );
    }
}

// ---- BDR 0009 S6 — no mouse event ever exits the app (property/invariant) ----

#[test]
fn mouse_mapper_never_yields_quit_and_update_never_sets_quit_cmd() {
    for search_active in [false, true] {
        for kind in all_mouse_event_kinds() {
            let mouse = mouse_event(kind);
            let Some(MouseIntent::Nav(msg)) = map_mouse_to_msg(mouse, search_active) else {
                continue;
            };
            assert!(
                !matches!(msg, Msg::Quit),
                "mapper must never yield Quit for {kind:?} (search_active={search_active})"
            );
            let model = make_list_model(&["PROJ-1"]);
            let (_, cmds) = update(model, msg);
            assert!(
                !cmds.contains(&Cmd::Quit),
                "update must never emit Cmd::Quit for a mouse-mapped msg ({kind:?})"
            );
        }
    }
}

// ---- BDR 0009 S3 — resolve_mouse_msg wires click resolution to the list screen only ----

#[test]
fn resolve_mouse_msg_click_on_list_screen_resolves_card_clicked() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let area = Rect::new(0, 0, 40, 20);
    let mouse = mouse_click_at(5, 1);

    assert!(matches!(
        resolve_mouse_msg(mouse, false, &model, area),
        Some(Msg::CardClicked(0))
    ));
}

#[test]
fn resolve_mouse_msg_click_on_detail_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    let area = Rect::new(0, 0, 40, 20);
    let mouse = mouse_click_at(5, 1);

    assert!(resolve_mouse_msg(mouse, false, &model, area).is_none());
}

// ---- issue 0033 D4 / BDR 0007 S7: footer_mode pure derivation ----

#[test]
fn footer_mode_list_screen_no_search_is_list() {
    let model = make_list_model(&["PROJ-1"]);
    assert_eq!(footer_mode(&model), FooterMode::List);
}

#[test]
fn footer_mode_list_screen_with_search_is_list_search() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("project = X".to_owned());
    assert_eq!(footer_mode(&model), FooterMode::ListSearch);
}

#[test]
fn footer_mode_detail_screen_no_focused_link_is_detail() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    assert_eq!(footer_mode(&model), FooterMode::Detail);
}

#[test]
fn footer_mode_detail_screen_with_focused_link_is_detail_link() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_links = vec!["https://example.com".to_owned()];
    model.detail_focused_link = Some(0);
    assert_eq!(footer_mode(&model), FooterMode::DetailLink);
}

// ---- issue 0033 D4 / BDR 0007 S7: view renders the mode-switched footer ----

#[test]
fn view_list_footer_switches_when_entering_search() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_list_model(&["PROJ-1"]);
    let (searching, _) = update(model, Msg::OpenSearch);

    let buf = render_to_buffer(&searching, 120, 20);
    let text = buffer_text(&buf);

    assert!(
        text.contains("Enter submit  Esc cancel  Backspace delete"),
        "entering search must switch the footer to the search hints; got: {text}"
    );
    assert!(
        !text.contains("↑/↓ navigate"),
        "the list footer must not remain visible once search is active; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_footer_switches_when_opening_detail() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));

    let buf = render_to_buffer(&model, 120, 30);
    let text = buffer_text(&buf);

    assert!(
        text.contains("↑/↓ j/k scroll  Esc/b back  q quit"),
        "opening detail must switch the footer to the detail hints; got: {text}"
    );
    assert!(
        !text.contains("↑/↓ navigate"),
        "the list footer must not remain visible on the detail screen; got: {text}"
    );

    set_language("en");
}

#[test]
fn view_detail_footer_switches_when_a_link_is_focused() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));

    let plain_text = buffer_text(&render_to_buffer(&model, 120, 30));
    assert!(plain_text.contains("↑/↓ j/k scroll  Esc/b back  q quit"));

    model.detail_links = vec!["https://example.com".to_owned()];
    model.detail_focused_link = Some(0);

    let linked_text = buffer_text(&render_to_buffer(&model, 120, 30));
    assert!(
        linked_text.contains("Tab next link") && linked_text.contains("Enter open"),
        "a focused link must switch the footer to the link-focus hints; got: {linked_text}"
    );

    set_language("en");
}

// ---- issue 0033 D4 / BDR 0007 S7: lesson-3345 guard — every advertised key is bound ----

fn assert_footer_hint_advertises_bound_key(
    hint: &str,
    substring: &str,
    key_code: KeyCode,
    search_active: bool,
) {
    assert!(
        hint.contains(substring),
        "hint {hint:?} must advertise {substring:?}"
    );
    let bound = if search_active {
        map_key_in_search_mode(key_code, KeyModifiers::NONE).is_some()
    } else {
        map_key_in_normal_mode(key_code, KeyModifiers::NONE).is_some()
    };
    assert!(
        bound,
        "{substring:?} in hint {hint:?} advertises {key_code:?} with no bound handler"
    );
}

#[test]
fn every_footer_mode_advertises_only_bound_keys() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let list_hint = view::footer_hint(FooterMode::List);
    assert_footer_hint_advertises_bound_key(&list_hint, "↑/↓", KeyCode::Down, false);
    assert_footer_hint_advertises_bound_key(&list_hint, "/", KeyCode::Char('/'), false);
    assert_footer_hint_advertises_bound_key(&list_hint, "Enter select", KeyCode::Enter, false);
    assert_footer_hint_advertises_bound_key(&list_hint, "Esc/b", KeyCode::Esc, false);
    assert_footer_hint_advertises_bound_key(&list_hint, "q quit", KeyCode::Char('q'), false);

    let search_hint = view::footer_hint(FooterMode::ListSearch);
    assert_footer_hint_advertises_bound_key(&search_hint, "Enter submit", KeyCode::Enter, true);
    assert_footer_hint_advertises_bound_key(&search_hint, "Esc cancel", KeyCode::Esc, true);
    assert_footer_hint_advertises_bound_key(&search_hint, "Backspace", KeyCode::Backspace, true);

    let detail_hint = view::footer_hint(FooterMode::Detail);
    assert_footer_hint_advertises_bound_key(&detail_hint, "j/k", KeyCode::Char('j'), false);
    assert_footer_hint_advertises_bound_key(&detail_hint, "Esc/b", KeyCode::Esc, false);
    assert_footer_hint_advertises_bound_key(&detail_hint, "q quit", KeyCode::Char('q'), false);

    let detail_link_hint = view::footer_hint(FooterMode::DetailLink);
    assert_footer_hint_advertises_bound_key(&detail_link_hint, "Tab", KeyCode::Tab, false);
    assert_footer_hint_advertises_bound_key(&detail_link_hint, "Enter open", KeyCode::Enter, false);

    set_language("en");
}

// ---- issue 0033 D4 / BDR 0007 S8: transient status — clear-before-process + set ----

#[test]
fn update_clears_a_standing_status_on_the_next_key_event() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.status = Some(StatusMsg {
        text: "stale".to_owned(),
        kind: StatusKind::Info,
    });

    let (next, _) = update(model, Msg::Down);

    assert!(
        next.status.is_none(),
        "any key-driven Msg must clear a standing status before it is processed"
    );
}

#[test]
fn update_copy_key_sets_an_info_status_confirmation() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::CopyKey);

    assert_eq!(cmds, vec![Cmd::CopyToClipboard("PROJ-1".to_owned())]);
    let status = next.status.expect("CopyKey must set a status message");
    assert_eq!(status.kind, StatusKind::Info);
    assert_eq!(status.text, "Copied ✓");

    set_language("en");
}

#[test]
fn update_copy_key_pt_br_translates_the_status_confirmation() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let model = make_list_model(&["PROJ-1"]);
    let (next, _) = update(model, Msg::CopyKey);

    let status = next.status.expect("CopyKey must set a status message");
    assert_eq!(status.text, "Copiado ✓");

    set_language("en");
}

#[test]
fn update_load_failed_sets_an_error_status_alongside_the_existing_banner() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, _) = update(model, Msg::LoadFailed("network down".to_owned()));

    let status = next.status.expect("LoadFailed must set a status message");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, "network down");
    assert_eq!(
        next.error.as_deref(),
        Some("network down"),
        "the existing search-JQL inline banner (BDR 0006 S5) must be unaffected"
    );
}

#[test]
fn update_reply_msgs_do_not_clear_a_standing_status() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.status = Some(StatusMsg {
        text: "still here".to_owned(),
        kind: StatusKind::Info,
    });

    let (next, _) = update(model, Msg::ListLoaded(vec![], None));

    assert_eq!(
        next.status.map(|s| s.text),
        Some("still here".to_owned()),
        "a background reply Msg must not clear a status set by a prior key event"
    );
}

// ---- e2-401-reauth-messaging: AC3 — a 401-driven LoadFailed surfaces the
// re-auth guidance (not a raw error) through model.error and model.status.
// Drives the same `Unauthorized -> reauth_message` seam every spawn site in
// `src/tui/shell.rs` (`spawn_load_list`/`spawn_load_detail`/`spawn_load_more`)
// builds `Msg::LoadFailed` from (ADR 0006/0008).

#[test]
fn update_load_failed_with_reauth_message_surfaces_guidance_in_error_and_status() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_list_model(&["PROJ-1"]);
    let guidance = crate::commands::reauth_message("work");

    let (next, cmds) = update(model, Msg::LoadFailed(guidance.clone()));

    assert_eq!(
        next.error.as_deref(),
        Some(guidance.as_str()),
        "a 401 LoadFailed must set the error banner to the re-auth guidance, not a raw error"
    );
    let status = next
        .status
        .expect("a 401 LoadFailed must set a status message");
    assert_eq!(
        status.kind,
        StatusKind::Error,
        "the re-auth guidance status must be StatusKind::Error"
    );
    assert_eq!(status.text, guidance);
    assert!(
        guidance.contains("jira setup add"),
        "the guidance text itself must carry the actionable re-auth instruction; got: {guidance:?}"
    );
    assert!(cmds.is_empty());

    set_language("en");
}

#[test]
fn update_load_failed_with_reauth_message_pt_br_translates_the_guidance() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let en_guidance = crate::commands::reauth_message("work");

    set_language("pt_BR");
    let model = make_list_model(&["PROJ-1"]);
    let pt_br_guidance = crate::commands::reauth_message("work");

    let (next, _) = update(model, Msg::LoadFailed(pt_br_guidance.clone()));

    assert_eq!(
        next.error.as_deref(),
        Some(pt_br_guidance.as_str()),
        "the pt_BR re-auth guidance must flow through unchanged into the error banner"
    );
    let status = next
        .status
        .expect("a 401 LoadFailed must set a status message");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, pt_br_guidance);
    assert_ne!(
        pt_br_guidance, en_guidance,
        "the pt_BR guidance must actually be translated, not the English fallback"
    );

    set_language("en");
}
