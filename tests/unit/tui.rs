use super::*;

use super::model::{
    entry_cmds, footer_mode, Compose, ComposeStatus, ComposeTarget, ConfirmDelete, FooterMode,
    ListOrigin, Selection, StatusKind, StatusMsg,
};
use super::shell::{
    handle_reply, map_key_in_compose_mode, map_key_in_confirm_mode, map_key_in_normal_mode,
    map_key_in_search_mode, map_mouse_to_msg, read_snapshot, resolve_mouse_msg, MouseIntent,
};
use super::view;
use crate::cli::{browse_tty_action, BrowseAction};
use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::{IssueRow, ProjectRow};
use crate::store::cache::{instances_key, TaskListCache};
use crate::test_support::*;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

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
        selection: None,
        list_origin: ListOrigin::Mine,
        projects: vec![],
        projects_selected: 0,
        compose: None,
        detail_focused_comment: None,
        current_account_id: None,
        confirm: None,
    }
}

fn make_projects_model(projects: Vec<ProjectRow>) -> Model {
    Model {
        screen: Screen::Projects,
        projects,
        projects_selected: 0,
        ..make_list_model(&[])
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

/// Finds `needle`'s first rendered cell's `(column, row)` — used by the ADR
/// 0018 modifier-click mapper tests to click exactly on a rendered `[url]`
/// token without hardcoding panel geometry.
fn find_text_position(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    let (width, height) = (buf.area.width as usize, buf.area.height as usize);
    for row in 0..height {
        let row_text: String = (0..width)
            .map(|col| buf[(col as u16, row as u16)].symbol().to_owned())
            .collect();
        if let Some(start) = row_text.find(needle) {
            return Some((start as u16, row as u16));
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
fn entry_cmds_warm_yields_load_myself_then_revalidate_list() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.revalidating = true;

    assert_eq!(
        entry_cmds(&model),
        vec![Cmd::LoadMyself, Cmd::RevalidateList]
    );
}

#[test]
fn entry_cmds_cold_yields_exactly_one_load_myself() {
    let model = make_list_model(&["PROJ-1"]);
    assert!(!model.revalidating);

    assert_eq!(entry_cmds(&model), vec![Cmd::LoadMyself]);
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

// ---- ADR 0018 §4 / BDR 0010 S5 — LinkClicked emits Cmd::OpenUrl on Detail ----

#[test]
fn update_link_clicked_on_detail_screen_emits_exactly_one_open_url_and_changes_no_state() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail_scroll = 3;
    let (next, cmds) = update(model, Msg::LinkClicked("https://example.com/y".to_owned()));

    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], Cmd::OpenUrl("https://example.com/y".to_owned()));
    assert_eq!(next.screen, Screen::Detail, "screen must be unchanged");
    assert_eq!(next.detail_scroll, 3, "scroll must be unchanged");
}

#[test]
fn update_link_clicked_on_list_screen_is_noop() {
    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::LinkClicked("https://example.com".to_owned()));

    assert_eq!(next.screen, Screen::List);
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
fn view_detail_renders_link_token_with_underlined_modifier_anchor_stays_plain() {
    let _lock = LANG_MUTEX.lock().unwrap();
    let mut model = make_list_model(&["PROJ-12"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_styled_description("PROJ-12"));

    let buf = render_to_buffer(&model, 120, 30);
    let anchor_style = style_at_text(&buf, "a link").expect("anchor text must appear in buffer");
    let token_style =
        style_at_text(&buf, "[https://example.com]").expect("[url] token must appear in buffer");

    assert!(
        !anchor_style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "anchor text must no longer carry link-derived underline: {anchor_style:?}"
    );
    assert!(
        token_style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "the [url] token must carry Modifier::UNDERLINED: {token_style:?}"
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
    let style = style_at_text(&buf, "[https://example.com/first]")
        .expect("focused link's [url] token must appear in buffer");

    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "the focused link's [url] token must carry Modifier::REVERSED: {style:?}"
    );
    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "the focused link's [url] token must still carry Modifier::UNDERLINED: {style:?}"
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
    let style = style_at_text(&buf, "[https://example.com/second]")
        .expect("non-focused link's [url] token must appear in buffer");

    assert!(
        !style
            .add_modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "non-focused link's [url] token must not carry Modifier::REVERSED: {style:?}"
    );
    assert!(
        style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "non-focused link's [url] token must still carry Modifier::UNDERLINED: {style:?}"
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
    let anchor_style =
        style_at_text(&buf, "linked comment text").expect("comment anchor text must appear");
    let token_style = style_at_text(&buf, "[https://example.com/comment]")
        .expect("comment [url] token must appear");

    assert!(
        !anchor_style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "comment anchor text must no longer carry link-derived underline: {anchor_style:?}"
    );
    assert!(
        token_style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED),
        "comment [url] token must carry Modifier::UNDERLINED: {token_style:?}"
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
        Some(MouseIntent::Click { x: 12, y: 7, .. })
    ));
}

// ---- ADR 0018 §4 / BDR 0010 S5-S8 — the click intent carries modifiers ----

#[test]
fn map_mouse_to_msg_left_down_carries_the_events_modifiers() {
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 12,
        row: 7,
        modifiers: KeyModifiers::CONTROL,
    };
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Click { x: 12, y: 7, modifiers }) if modifiers == KeyModifiers::CONTROL
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
fn map_mouse_to_msg_ignores_non_left_button_and_moved() {
    for kind in [
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Drag(MouseButton::Right),
        MouseEventKind::Up(MouseButton::Right),
        MouseEventKind::Moved,
    ] {
        assert!(
            map_mouse_to_msg(mouse_event(kind), false).is_none(),
            "{kind:?} must not map to any intent"
        );
    }
}

// ---- ADR 0019 §3 / BDR 0011 S1-S4 — Drag/Release mouse mapping ----

#[test]
fn map_mouse_to_msg_left_drag_is_drag_intent_with_coordinates() {
    let mouse = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 9,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Drag { x: 9, y: 4, .. })
    ));
}

#[test]
fn map_mouse_to_msg_left_up_is_release_intent() {
    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 9,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
        map_mouse_to_msg(mouse, false),
        Some(MouseIntent::Release {
            modifiers: KeyModifiers::NONE
        })
    ));
}

#[test]
fn map_mouse_to_msg_search_active_swallows_drag_and_release() {
    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    assert!(map_mouse_to_msg(drag, true).is_none());
    assert!(map_mouse_to_msg(up, true).is_none());
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

// ---- ADR 0018 §4 / BDR 0010 S5-S8 — modifier-gated Detail link activation ----

fn ctrl_click_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::CONTROL,
    }
}

fn super_click_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::SUPER,
    }
}

fn make_detail_model_with_link() -> Model {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_styled_description("PROJ-1"));
    model.detail_links = vec!["https://example.com".to_owned()];
    model.detail_focused_link = Some(0);
    model
}

#[test]
fn resolve_mouse_msg_ctrl_click_on_detail_link_token_resolves_link_clicked() {
    let model = make_detail_model_with_link();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) =
        find_text_position(&buf, "[https://example.com]").expect("the [url] token must render");

    assert!(matches!(
        resolve_mouse_msg(ctrl_click_at(col, row), false, &model, area),
        Some(Msg::LinkClicked(ref href)) if href == "https://example.com"
    ));
}

#[test]
fn resolve_mouse_msg_super_click_on_detail_link_token_resolves_link_clicked() {
    let model = make_detail_model_with_link();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) =
        find_text_position(&buf, "[https://example.com]").expect("the [url] token must render");

    assert!(matches!(
        resolve_mouse_msg(super_click_at(col, row), false, &model, area),
        Some(Msg::LinkClicked(ref href)) if href == "https://example.com"
    ));
}

#[test]
fn resolve_mouse_msg_plain_click_on_detail_link_token_never_activates_the_link() {
    let model = make_detail_model_with_link();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) =
        find_text_position(&buf, "[https://example.com]").expect("the [url] token must render");

    // ADR 0019 §3 (BDR 0011 S1): a plain click on the detail body now anchors
    // a selection rather than being a pure no-op — it must still never
    // resolve to link activation (that stays modifier-gated, BDR 0010 S5).
    assert!(!matches!(
        resolve_mouse_msg(mouse_click_at(col, row), false, &model, area),
        Some(Msg::LinkClicked(_))
    ));
}

#[test]
fn resolve_mouse_msg_ctrl_click_on_list_screen_behaves_like_a_plain_click() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let area = Rect::new(0, 0, 40, 20);

    assert!(matches!(
        resolve_mouse_msg(ctrl_click_at(5, 1), false, &model, area),
        Some(Msg::CardClicked(0))
    ));
}

#[test]
fn resolve_mouse_msg_super_click_on_list_screen_behaves_like_a_plain_click() {
    let model = make_list_model(&["PROJ-1", "PROJ-2"]);
    let area = Rect::new(0, 0, 40, 20);

    assert!(matches!(
        resolve_mouse_msg(super_click_at(5, 1), false, &model, area),
        Some(Msg::CardClicked(0))
    ));
}

// ---- BDR 0009 S6 / BDR 0010 S8 — the no-exit property extends to modifier-click variants ----

#[test]
fn modifier_click_variants_never_yield_quit_on_either_screen() {
    let area = Rect::new(0, 0, 40, 20);
    let modifiers_variants = [
        KeyModifiers::NONE,
        KeyModifiers::CONTROL,
        KeyModifiers::SUPER,
        KeyModifiers::CONTROL | KeyModifiers::SUPER,
    ];

    for modifiers in modifiers_variants {
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 1,
            modifiers,
        };

        let list_model = make_list_model(&["PROJ-1"]);
        if let Some(msg) = resolve_mouse_msg(mouse, false, &list_model, area) {
            assert!(
                !matches!(msg, Msg::Quit),
                "resolve_mouse_msg must never yield Quit on List (modifiers={modifiers:?})"
            );
            let (_, cmds) = update(list_model, msg);
            assert!(
                !cmds.contains(&Cmd::Quit),
                "update must never emit Cmd::Quit for a List modifier-click (modifiers={modifiers:?})"
            );
        }

        let mut detail_model = make_list_model(&["PROJ-1"]);
        detail_model.screen = Screen::Detail;
        if let Some(msg) = resolve_mouse_msg(mouse, false, &detail_model, area) {
            assert!(
                !matches!(msg, Msg::Quit),
                "resolve_mouse_msg must never yield Quit on Detail (modifiers={modifiers:?})"
            );
            let (_, cmds) = update(detail_model, msg);
            assert!(
                !cmds.contains(&Cmd::Quit),
                "update must never emit Cmd::Quit for a Detail modifier-click (modifiers={modifiers:?})"
            );
        }
    }
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

// ---- b3-app-managed-selection / ADR 0019 / BDR 0011 S1-S4 — pure Selection
// state machine (SelStart/SelDrag/SelEnd) ----

fn make_detail_model_for_selection() -> Model {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));
    model
}

#[test]
fn update_sel_start_anchors_selection_replacing_any_previous() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (9, 9),
        cursor: (9, 9),
        dragged: true,
    });

    let (next, cmds) = update(model, Msg::SelStart((1, 2)));

    let selection = next.selection.expect("SelStart must set a selection");
    assert_eq!(selection.anchor, (1, 2));
    assert_eq!(selection.cursor, (1, 2));
    assert!(!selection.dragged, "a fresh SelStart must not be dragged");
    assert!(cmds.is_empty());
}

#[test]
fn update_sel_start_on_list_screen_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::SelStart((0, 0)));

    assert!(next.selection.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_sel_drag_extends_cursor_and_marks_dragged_keeping_anchor() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 0),
        dragged: false,
    });

    let (next, cmds) = update(model, Msg::SelDrag((2, 3)));

    let selection = next.selection.expect("selection must remain active");
    assert_eq!(selection.anchor, (0, 0), "anchor must not move on drag");
    assert_eq!(selection.cursor, (2, 3));
    assert!(selection.dragged);
    assert!(cmds.is_empty());
}

#[test]
fn update_sel_drag_with_no_active_selection_is_noop() {
    let model = make_detail_model_for_selection();
    assert!(model.selection.is_none());

    let (next, cmds) = update(model, Msg::SelDrag((2, 3)));

    assert!(next.selection.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_sel_drag_on_list_screen_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::SelDrag((2, 3)));

    assert!(next.selection.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_sel_end_some_emits_exactly_one_copy_to_clipboard_sets_copied_status_and_keeps_selection()
{
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 5),
        dragged: true,
    });

    let (next, cmds) = update(model, Msg::SelEnd(Some("hello".to_owned())));

    assert_eq!(cmds, vec![Cmd::CopyToClipboard("hello".to_owned())]);
    assert_eq!(
        next.status,
        Some(StatusMsg {
            text: "Copied ✓".to_owned(),
            kind: StatusKind::Info,
        }),
        "SelEnd(Some) must reuse the existing 'Copied' status contract"
    );
    let selection = next
        .selection
        .expect("SelEnd(Some) must keep the highlight visible");
    assert!(selection.dragged);
}

#[test]
fn update_sel_end_none_clears_selection_with_no_cmd_and_no_status() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 0),
        dragged: false,
    });

    let (next, cmds) = update(model, Msg::SelEnd(None));

    assert!(
        next.selection.is_none(),
        "a plain click must clear the selection"
    );
    assert!(cmds.is_empty(), "a plain click must copy nothing");
    assert!(
        next.status.is_none(),
        "a plain click must never set a status"
    );
    assert_eq!(
        next.screen,
        Screen::Detail,
        "a plain click must never navigate"
    );
}

#[test]
fn update_sel_end_on_list_screen_is_noop() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::SelEnd(Some("x".to_owned())));

    assert!(next.selection.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn update_back_clears_an_active_selection() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 3),
        dragged: true,
    });

    let (next, _) = update(model, Msg::Back);

    assert!(
        next.selection.is_none(),
        "Back must clear a stale selection so it never survives a screen change"
    );
}

// ---- ADR 0019 §3-4 / BDR 0011 S1-S4 — Drag/Release mouse resolution ----

fn drag_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn release_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn resolve_mouse_msg_plain_down_on_detail_body_anchors_selection() {
    let model = make_detail_model_for_selection();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) = find_text_position(&buf, "Flattened description here.")
        .expect("description text must render");

    assert!(matches!(
        resolve_mouse_msg(mouse_click_at(col, row), false, &model, area),
        Some(Msg::SelStart(_))
    ));
}

#[test]
fn resolve_mouse_msg_drag_on_detail_body_resolves_sel_drag() {
    let model = make_detail_model_for_selection();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) = find_text_position(&buf, "Flattened description here.")
        .expect("description text must render");

    assert!(matches!(
        resolve_mouse_msg(drag_at(col, row), false, &model, area),
        Some(Msg::SelDrag(_))
    ));
}

#[test]
fn resolve_mouse_msg_drag_on_list_screen_is_unmapped() {
    let model = make_list_model(&["PROJ-1"]);
    let area = Rect::new(0, 0, 40, 20);

    assert!(resolve_mouse_msg(drag_at(5, 1), false, &model, area).is_none());
}

#[test]
fn resolve_mouse_msg_release_on_list_screen_is_unmapped() {
    let model = make_list_model(&["PROJ-1"]);
    let area = Rect::new(0, 0, 40, 20);

    assert!(resolve_mouse_msg(release_at(5, 1), false, &model, area).is_none());
}

#[test]
fn resolve_mouse_msg_release_after_a_drag_extracts_text() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 3),
        dragged: true,
    });
    let area = Rect::new(0, 0, 60, 20);

    let msg = resolve_mouse_msg(release_at(0, 0), false, &model, area);
    assert!(
        matches!(msg, Some(Msg::SelEnd(Some(_)))),
        "a release after a drag must resolve to SelEnd(Some(text))"
    );
}

#[test]
fn resolve_mouse_msg_release_without_a_drag_resolves_sel_end_none() {
    let mut model = make_detail_model_for_selection();
    model.selection = Some(Selection {
        anchor: (0, 0),
        cursor: (0, 0),
        dragged: false,
    });
    let area = Rect::new(0, 0, 60, 20);

    assert!(matches!(
        resolve_mouse_msg(release_at(0, 0), false, &model, area),
        Some(Msg::SelEnd(None))
    ));
}

#[test]
fn resolve_mouse_msg_ctrl_down_on_detail_never_starts_selection_still_resolves_link() {
    let model = make_detail_model_with_link();
    let area = Rect::new(0, 0, 60, 20);
    let buf = render_to_buffer(&model, 60, 20);
    let (col, row) =
        find_text_position(&buf, "[https://example.com]").expect("the [url] token must render");

    let msg = resolve_mouse_msg(ctrl_click_at(col, row), false, &model, area);
    assert!(
        !matches!(msg, Some(Msg::SelStart(_))),
        "a Ctrl-held down must never anchor a selection (BDR 0011 S4)"
    );
    assert!(matches!(msg, Some(Msg::LinkClicked(ref href)) if href == "https://example.com"));
}

#[test]
fn resolve_mouse_msg_modifier_drag_and_release_on_detail_are_noops() {
    let model = make_detail_model_for_selection();
    let area = Rect::new(0, 0, 60, 20);

    let ctrl_drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 5,
        row: 5,
        modifiers: KeyModifiers::CONTROL,
    };
    let ctrl_release = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 5,
        row: 5,
        modifiers: KeyModifiers::CONTROL,
    };

    assert!(resolve_mouse_msg(ctrl_drag, false, &model, area).is_none());
    assert!(resolve_mouse_msg(ctrl_release, false, &model, area).is_none());
}

// ---- BDR 0009 S6 / BDR 0011 — the no-exit property extends to Drag/Release ----

#[test]
fn drag_and_release_variants_never_yield_quit_on_either_screen() {
    let area = Rect::new(0, 0, 60, 20);
    let modifiers_variants = [
        KeyModifiers::NONE,
        KeyModifiers::CONTROL,
        KeyModifiers::SUPER,
    ];

    for modifiers in modifiers_variants {
        for kind in [
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
        ] {
            let mouse = MouseEvent {
                kind,
                column: 5,
                row: 1,
                modifiers,
            };

            let list_model = make_list_model(&["PROJ-1"]);
            if let Some(msg) = resolve_mouse_msg(mouse, false, &list_model, area) {
                assert!(!matches!(msg, Msg::Quit));
                let (_, cmds) = update(list_model, msg);
                assert!(!cmds.contains(&Cmd::Quit));
            }

            let detail_model = make_detail_model_for_selection();
            if let Some(msg) = resolve_mouse_msg(mouse, false, &detail_model, area) {
                assert!(!matches!(msg, Msg::Quit));
                let (_, cmds) = update(detail_model, msg);
                assert!(!cmds.contains(&Cmd::Quit));
            }
        }
    }
}

// ---- ADR 0020 / BDR 0012 S6-S7: attachment rows reuse B2b/B3 machinery
// with zero new click/selection code ----

fn make_detail_model_with_attachment() -> Model {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(crate::models::Issue {
        attachments: vec![attachment("a.pdf", "https://example.com/a.pdf", None, None)],
        ..make_issue("PROJ-1")
    });
    model
}

#[test]
fn resolve_mouse_msg_plain_click_on_attachment_row_anchors_selection_never_opens() {
    let model = make_detail_model_with_attachment();
    let area = Rect::new(0, 0, 60, 30);
    let buf = render_to_buffer(&model, 60, 30);
    let (col, row) =
        find_text_position(&buf, "[1] ↗ a.pdf").expect("the attachment row must render");

    let msg = resolve_mouse_msg(mouse_click_at(col, row), false, &model, area);
    assert!(
        matches!(msg, Some(Msg::SelStart(_))),
        "a plain click over an attachment row must go down the B3 selection path (SelStart), \
         never open a URL"
    );
}

#[test]
fn selection_text_over_an_attachment_row_extracts_the_rows_logical_text() {
    let mut model = make_detail_model_with_attachment();
    let area = Rect::new(0, 0, 60, 30);
    let buf = render_to_buffer(&model, 60, 30);
    let (col, row) =
        find_text_position(&buf, "[1] ↗ a.pdf").expect("the attachment row must render");

    let (line, _) = view::detail_pos_at(&model, area, col, row)
        .expect("the attachment row must resolve to a logical position");
    model.selection = Some(Selection {
        anchor: (line, 0),
        cursor: (line, "[1] ↗ a.pdf".chars().count()),
        dragged: true,
    });

    assert_eq!(
        view::selection_text(&model),
        Some("[1] ↗ a.pdf".to_owned()),
        "selection over an attachment row must extract exactly its logical text"
    );
}

#[test]
fn update_link_clicked_with_an_attachment_url_on_detail_emits_exactly_one_open_url() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    let (next, cmds) = update(
        model,
        Msg::LinkClicked("https://example.com/a.pdf".to_owned()),
    );

    assert_eq!(
        cmds,
        vec![Cmd::OpenUrl("https://example.com/a.pdf".to_owned())]
    );
    assert_eq!(next.screen, Screen::Detail, "screen must be unchanged");
}

// ---- ADR 0021 / BDR 0013 S1: 'p' opens the Projects screen ----

#[test]
fn update_open_projects_from_list_opens_screen_and_emits_load_projects() {
    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::OpenProjects);

    assert_eq!(next.screen, Screen::Projects);
    assert_eq!(next.projects_selected, 0);
    assert_eq!(cmds, vec![Cmd::LoadProjects]);
}

#[test]
fn update_open_projects_keeps_existing_projects_rows_while_loading() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.projects = vec![project_row("OLD", "Old Project")];

    let (next, cmds) = update(model, Msg::OpenProjects);

    assert_eq!(
        next.projects,
        vec![project_row("OLD", "Old Project")],
        "existing projects rows must be kept while the fresh fetch is in flight"
    );
    assert_eq!(cmds, vec![Cmd::LoadProjects]);
}

#[test]
fn update_open_projects_on_detail_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let (next, cmds) = update(model, Msg::OpenProjects);

    assert_eq!(next.screen, Screen::Detail, "'p' on Detail must be inert");
    assert!(cmds.is_empty());
}

#[test]
fn update_open_projects_while_search_active_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.search = Some("proj".to_owned());

    let (next, cmds) = update(model, Msg::OpenProjects);

    assert_eq!(
        next.screen,
        Screen::List,
        "OpenProjects while search is active must not switch screens"
    );
    assert!(cmds.is_empty());
}

#[test]
fn map_key_in_normal_mode_p_is_open_projects() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('p'), KeyModifiers::NONE),
        Some(Msg::OpenProjects)
    ));
}

#[test]
fn map_key_in_search_mode_p_types_into_the_query() {
    assert!(matches!(
        map_key_in_search_mode(KeyCode::Char('p'), KeyModifiers::NONE),
        Some(Msg::SearchInput('p'))
    ));
}

#[test]
fn update_projects_loaded_sets_rows_and_clamps_selection() {
    let mut model = make_projects_model(vec![]);
    model.projects_selected = 5;
    model.status = Some(StatusMsg {
        text: "Loading…".to_owned(),
        kind: StatusKind::Info,
    });

    let rows = vec![
        project_row("ALPHA", "Alpha Project"),
        project_row("BETA", "Beta Project"),
    ];
    let (next, cmds) = update(model, Msg::ProjectsLoaded(rows.clone()));

    assert_eq!(next.projects, rows);
    assert_eq!(
        next.projects_selected, 1,
        "selection must clamp to the new last index"
    );
    assert!(
        next.status.is_none(),
        "ProjectsLoaded must clear the loading status"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_projects_loaded_off_projects_screen_updates_data_but_never_changes_screen() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let rows = vec![project_row("ALPHA", "Alpha Project")];
    let (next, cmds) = update(model, Msg::ProjectsLoaded(rows.clone()));

    assert_eq!(
        next.projects, rows,
        "a late ProjectsLoaded still updates the data (harmless)"
    );
    assert_eq!(
        next.screen,
        Screen::Detail,
        "a late ProjectsLoaded must never change the screen"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_projects_failed_sets_error_status_and_stays_on_projects() {
    let model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);

    let (next, cmds) = update(model, Msg::ProjectsFailed("network unreachable".to_owned()));

    assert_eq!(
        next.screen,
        Screen::Projects,
        "failure must stay on Projects"
    );
    let status = next
        .status
        .expect("ProjectsFailed must set a status message");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, "network unreachable");
    assert!(cmds.is_empty());
}

#[test]
fn update_projects_failed_with_reauth_message_surfaces_the_e2_guidance() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let model = make_projects_model(vec![]);
    let guidance = crate::commands::reauth_message("work");

    let (next, cmds) = update(model, Msg::ProjectsFailed(guidance.clone()));

    let status = next
        .status
        .expect("a 401 ProjectsFailed must set a status message");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, guidance);
    assert!(
        guidance.contains("jira setup add"),
        "the guidance text must carry the actionable re-auth instruction; got: {guidance:?}"
    );
    assert_eq!(next.screen, Screen::Projects, "screen must be unchanged");
    assert!(cmds.is_empty());

    set_language("en");
}

#[tokio::test]
async fn run_list_projects_success_returns_rows() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [{ "key": "ALPHA", "name": "Alpha Project" }]
        })))
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

    let result = run_list_projects(&instance).await.expect("must succeed");

    assert_eq!(result, vec![project_row("ALPHA", "Alpha Project")]);
    server.verify().await;
}

#[tokio::test]
async fn run_list_projects_401_maps_to_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(401))
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

    let result = run_list_projects(&instance).await;

    assert!(matches!(
        result,
        Err(crate::client::ClientError::Unauthorized { .. })
    ));
    server.verify().await;
}

// ---- ADR 0021 / BDR 0013 S2: navigation mirrors the list ----

#[test]
fn update_down_on_projects_increments_projects_selected() {
    let model = make_projects_model(vec![
        project_row("A", "A"),
        project_row("B", "B"),
        project_row("C", "C"),
    ]);
    let (next, cmds) = update(model, Msg::Down);

    assert_eq!(next.projects_selected, 1);
    assert!(cmds.is_empty());
}

#[test]
fn update_down_on_projects_clamps_at_last_row() {
    let mut model = make_projects_model(vec![project_row("A", "A"), project_row("B", "B")]);
    model.projects_selected = 1;
    let (next, cmds) = update(model, Msg::Down);

    assert_eq!(next.projects_selected, 1, "Down at last row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_projects_decrements_projects_selected() {
    let mut model = make_projects_model(vec![project_row("A", "A"), project_row("B", "B")]);
    model.projects_selected = 1;
    let (next, cmds) = update(model, Msg::Up);

    assert_eq!(next.projects_selected, 0);
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_projects_clamps_at_zero() {
    let model = make_projects_model(vec![project_row("A", "A")]);
    let (next, cmds) = update(model, Msg::Up);

    assert_eq!(next.projects_selected, 0, "Up at first row must clamp");
    assert!(cmds.is_empty());
}

#[test]
fn update_down_on_empty_projects_is_noop() {
    let model = make_projects_model(vec![]);
    let (next, cmds) = update(model, Msg::Down);

    assert_eq!(next.projects_selected, 0);
    assert!(cmds.is_empty());
}

#[test]
fn update_up_on_empty_projects_is_noop() {
    let model = make_projects_model(vec![]);
    let (next, cmds) = update(model, Msg::Up);

    assert_eq!(next.projects_selected, 0);
    assert!(cmds.is_empty());
}

#[test]
fn update_project_clicked_in_range_selects_and_drills_in() {
    let model = make_projects_model(vec![
        project_row("A", "A Project"),
        project_row("B", "B Project"),
    ]);

    let (next, cmds) = update(model, Msg::ProjectClicked(1));

    assert_eq!(next.projects_selected, 1);
    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Project("B".to_owned()));
    assert_eq!(
        cmds,
        vec![Cmd::LoadList(
            "project = B ORDER BY updated DESC".to_owned()
        )]
    );
}

#[test]
fn update_project_clicked_out_of_range_is_noop_no_panic() {
    let model = make_projects_model(vec![project_row("A", "A Project")]);
    let (next, cmds) = update(model, Msg::ProjectClicked(5));

    assert_eq!(next.screen, Screen::Projects);
    assert_eq!(next.projects_selected, 0);
    assert!(cmds.is_empty());
}

#[test]
fn update_project_clicked_on_empty_projects_is_noop_no_panic() {
    let model = make_projects_model(vec![]);
    let (next, cmds) = update(model, Msg::ProjectClicked(0));

    assert_eq!(next.screen, Screen::Projects);
    assert!(cmds.is_empty());
}

#[test]
fn update_project_clicked_on_non_projects_screen_is_noop() {
    let model = make_list_model(&["PROJ-1"]);
    let (next, cmds) = update(model, Msg::ProjectClicked(0));

    assert_eq!(next.screen, Screen::List);
    assert!(cmds.is_empty());
}

// ---- BDR 0009 S6 / BDR 0013 S2 — the no-exit property extends to the
// Projects screen ----

#[test]
fn no_msg_on_projects_screen_ever_emits_cmd_quit() {
    let projects = vec![project_row("A", "A Project"), project_row("B", "B Project")];
    let msgs = vec![
        Msg::Up,
        Msg::Down,
        Msg::Select,
        Msg::Back,
        Msg::ProjectClicked(0),
        Msg::ProjectClicked(99),
        Msg::OpenProjects,
        Msg::ProjectsLoaded(vec![project_row("C", "C Project")]),
        Msg::ProjectsFailed("boom".to_owned()),
    ];

    for msg in msgs {
        let model = make_projects_model(projects.clone());
        let (_, cmds) = update(model, msg);
        assert!(
            !cmds.contains(&Cmd::Quit),
            "no Msg on the Projects screen may ever emit Cmd::Quit"
        );
    }
}

// ---- ADR 0021 / BDR 0013 S3: drill-in loads the project's issues ----

#[test]
fn update_select_projects_sets_origin_jql_clears_list_state_and_emits_load_list() {
    let mut model = make_projects_model(vec![
        project_row("ALPHA", "Alpha Project"),
        project_row("BETA", "Beta Project"),
    ]);
    model.projects_selected = 0;
    model.rows = make_rows(&["OLD-1", "OLD-2"]);
    model.selected = 1;
    model.next_page_token = Some("stale-token".to_owned());
    model.search = Some("stale search".to_owned());
    model.error = Some("stale error".to_owned());

    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Project("ALPHA".to_owned()));
    assert_eq!(next.jql, "project = ALPHA ORDER BY updated DESC");
    assert!(next.rows.is_empty(), "drill-in must clear the prior rows");
    assert_eq!(next.selected, 0);
    assert!(next.next_page_token.is_none());
    assert!(next.search.is_none());
    assert!(next.error.is_none());
    assert_eq!(
        cmds,
        vec![Cmd::LoadList(
            "project = ALPHA ORDER BY updated DESC".to_owned()
        )],
        "drill-in must emit exactly one LoadList"
    );
}

#[test]
fn update_select_projects_with_empty_projects_is_noop() {
    let model = make_projects_model(vec![]);
    let (next, cmds) = update(model, Msg::Select);

    assert_eq!(next.screen, Screen::Projects);
    assert!(cmds.is_empty());
}

#[test]
fn drilled_in_project_list_pagination_still_works() {
    let mut model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    model.projects_selected = 0;
    let (drilled, _) = update(model, Msg::Select);

    let (loaded, _) = update(
        drilled,
        Msg::ListLoaded(make_rows(&["ALPHA-1"]), Some("page-2".to_owned())),
    );
    assert_eq!(loaded.list_origin, ListOrigin::Project("ALPHA".to_owned()));

    let (next, cmds) = update(loaded, Msg::LoadMore);
    assert_eq!(
        cmds,
        vec![Cmd::LoadMore(
            "project = ALPHA ORDER BY updated DESC".to_owned(),
            "page-2".to_owned()
        )],
        "LoadMore on a project-origin list must still fire using the project's JQL"
    );
    assert_eq!(next.list_origin, ListOrigin::Project("ALPHA".to_owned()));
}

#[test]
fn drilled_in_project_list_search_still_works() {
    let model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    let (drilled, _) = update(model, Msg::Select);
    let (loaded, _) = update(drilled, Msg::ListLoaded(make_rows(&["ALPHA-1"]), None));

    let (searching, _) = update(loaded, Msg::OpenSearch);
    let (next, cmds) = update(searching, Msg::SearchInput('x'));
    assert_eq!(next.search.as_deref(), Some("x"));
    assert!(cmds.is_empty());
}

#[test]
fn drilled_in_project_list_select_opens_detail_like_normal() {
    let model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    let (drilled, _) = update(model, Msg::Select);
    let (loaded, _) = update(drilled, Msg::ListLoaded(make_rows(&["ALPHA-1"]), None));

    let (next, cmds) = update(loaded, Msg::Select);

    assert_eq!(next.screen, Screen::Detail);
    assert_eq!(cmds, vec![Cmd::LoadDetail("ALPHA-1".to_owned())]);
}

// ---- ADR 0021 / BDR 0013 S4: back pops the axis ----

#[test]
fn back_from_list_with_project_origin_returns_to_projects_with_rows_retained_no_cmd() {
    let mut model = make_list_model(&["ALPHA-1", "ALPHA-2"]);
    model.list_origin = ListOrigin::Project("ALPHA".to_owned());
    model.projects = vec![project_row("ALPHA", "Alpha Project")];
    model.projects_selected = 0;

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::Projects);
    assert_eq!(
        next.projects,
        vec![project_row("ALPHA", "Alpha Project")],
        "the projects rows must be retained, not refetched"
    );
    assert!(cmds.is_empty(), "returning to Projects must emit no Cmd");
}

#[test]
fn back_from_projects_with_project_origin_restores_mine_list_reloaded() {
    let mut model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    model.list_origin = ListOrigin::Project("ALPHA".to_owned());
    model.rows = vec![];
    model.jql = "project = ALPHA ORDER BY updated DESC".to_owned();

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Mine);
    assert_eq!(next.jql, crate::commands::MINE_JQL);
    assert!(next.rows.is_empty());
    assert_eq!(
        cmds,
        vec![Cmd::LoadList(crate::commands::MINE_JQL.to_owned())],
        "restoring the mine list must emit exactly one reload"
    );
}

#[test]
fn back_from_projects_with_mine_origin_returns_to_list_with_rows_intact_no_cmd() {
    let mut model = make_projects_model(vec![project_row("ALPHA", "Alpha Project")]);
    model.rows = make_rows(&["MINE-1", "MINE-2"]);
    assert_eq!(model.list_origin, ListOrigin::Mine);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Mine);
    assert_eq!(
        next.rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        vec!["MINE-1".to_owned(), "MINE-2".to_owned()],
        "the mine rows were never replaced, so nothing needs reloading"
    );
    assert!(
        cmds.is_empty(),
        "nothing was replaced, so no Cmd is emitted"
    );
}

#[test]
fn back_from_list_with_mine_origin_is_still_a_noop() {
    let model = make_list_model(&["PROJ-1"]);
    assert_eq!(model.list_origin, ListOrigin::Mine);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert!(cmds.is_empty());
}

#[test]
fn quit_from_projects_screen_emits_cmd_quit() {
    let model = make_projects_model(vec![project_row("A", "A Project")]);
    let (_, cmds) = update(model, Msg::Quit);

    assert!(cmds.contains(&Cmd::Quit));
}

// ---- ADR 0021 §7 / BDR 0013 S6: the mine SWR snapshot stays clean ----

#[test]
fn project_issue_list_loaded_never_writes_the_mine_snapshot() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    let mut model = make_list_model(&[]);
    model.list_origin = ListOrigin::Project("ALPHA".to_owned());

    let _ = handle_reply(
        Msg::ListLoaded(make_rows(&["ALPHA-1"]), None),
        model,
        &instance,
        &cache,
        &tx,
    );

    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    let stored = list_cache
        .read("mine", &key, 3600)
        .expect("read must not error");
    assert!(
        stored.is_none(),
        "a project ListLoaded must never write the mine-scope snapshot"
    );
}

#[test]
fn revalidation_loaded_still_writes_the_mine_snapshot_unaffected() {
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());
    let instance = make_test_instance();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    let mut model = make_list_model(&["MINE-1"]);
    model.revalidating = true;

    let _ = handle_reply(
        Msg::RevalidationLoaded(make_rows(&["MINE-2"]), None),
        model,
        &instance,
        &cache,
        &tx,
    );

    let list_cache = TaskListCache::new(store.conn());
    let key = instances_key(std::slice::from_ref(&instance));
    let stored = list_cache
        .read("mine", &key, 3600)
        .expect("read must not error")
        .expect("RevalidationLoaded must still write the mine-scope snapshot");
    assert!(stored.contains("MINE-2"));
}

// ---- c3b-comment-compose / ADR 0024 / BDR 0015 S1-S8 — compose state
// machine, input-leakage guards, 'c' detail-only scoping ----

fn make_compose_detail_model() -> Model {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue("PROJ-1"));
    model
}

fn make_composing_detail_model(buffer: &str) -> Model {
    let mut model = make_compose_detail_model();
    model.detail_scroll = 2;
    model.compose = Some(Compose {
        buffer: buffer.to_owned(),
        status: ComposeStatus::Idle,
        target: ComposeTarget::New,
    });
    model
}

fn make_editing_detail_model(buffer: &str, comment_id: &str) -> Model {
    let mut model = make_compose_detail_model();
    model.detail_scroll = 2;
    model.compose = Some(Compose {
        buffer: buffer.to_owned(),
        status: ComposeStatus::Idle,
        target: ComposeTarget::Edit {
            comment_id: comment_id.to_owned(),
        },
    });
    model
}

// ---- S1, S8: OpenCompose is Detail-only, and only with a loaded issue ----

#[test]
fn open_compose_on_detail_with_loaded_issue_opens_an_empty_idle_compose() {
    let model = make_compose_detail_model();

    let (next, cmds) = update(model, Msg::OpenCompose);

    let compose = next
        .compose
        .expect("OpenCompose on Detail with a loaded issue must open the compose");
    assert_eq!(compose.buffer, "");
    assert_eq!(compose.status, ComposeStatus::Idle);
    assert!(cmds.is_empty());
}

#[test]
fn open_compose_on_list_screen_opens_no_compose() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::OpenCompose);

    assert!(next.compose.is_none(), "'c' on List must open no compose");
    assert!(cmds.is_empty());
}

#[test]
fn open_compose_on_projects_screen_opens_no_compose() {
    let model = make_projects_model(vec![project_row("A", "A Project")]);

    let (next, cmds) = update(model, Msg::OpenCompose);

    assert!(
        next.compose.is_none(),
        "'c' on Projects must open no compose"
    );
    assert!(cmds.is_empty());
}

#[test]
fn open_compose_on_detail_screen_still_loading_opens_no_compose() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = None;

    let (next, cmds) = update(model, Msg::OpenCompose);

    assert!(
        next.compose.is_none(),
        "a Detail screen with no loaded issue has nothing to attach a comment to"
    );
    assert!(cmds.is_empty());
}

// ---- S1: typing builds a multi-line buffer; Enter is a newline, never a submit ----

#[test]
fn typing_and_enter_build_a_two_line_buffer_with_no_cmd() {
    let model = make_compose_detail_model();
    let (model, _) = update(model, Msg::OpenCompose);
    let (model, _) = update(model, Msg::ComposeInput('L'));
    let (model, _) = update(model, Msg::ComposeInput('1'));
    let (model, _) = update(model, Msg::ComposeNewline);
    let (model, cmds) = update(model, Msg::ComposeInput('2'));

    let compose = model
        .compose
        .expect("compose must remain open while typing");
    assert_eq!(compose.buffer, "L1\n2");
    assert!(
        compose.buffer.contains('\n'),
        "Enter must insert a newline, not submit"
    );
    assert!(cmds.is_empty());
}

#[test]
fn compose_backspace_pops_the_last_character() {
    let model = make_composing_detail_model("ab");

    let (next, cmds) = update(model, Msg::ComposeBackspace);

    assert_eq!(next.compose.unwrap().buffer, "a");
    assert!(cmds.is_empty());
}

#[test]
fn compose_input_with_no_compose_open_is_noop() {
    let model = make_compose_detail_model();

    let (next, cmds) = update(model, Msg::ComposeInput('x'));

    assert!(next.compose.is_none());
    assert!(cmds.is_empty());
}

// ---- S2, S7: Ctrl+S submit gate — non-empty submits exactly once; empty/whitespace never does ----

#[test]
fn submit_compose_with_non_empty_buffer_emits_exactly_one_submit_comment_and_sets_submitting() {
    let model = make_composing_detail_model("hi");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert_eq!(
        cmds,
        vec![Cmd::SubmitComment {
            key: "PROJ-1".to_owned(),
            body: "hi".to_owned(),
        }],
        "a non-empty buffer must emit exactly one SubmitComment for the open key"
    );
    let compose = next
        .compose
        .expect("the compose must stay open while submitting");
    assert_eq!(compose.status, ComposeStatus::Submitting);
    assert_eq!(compose.buffer, "hi", "the buffer must be unchanged");
}

#[test]
fn submit_compose_with_empty_buffer_emits_no_cmd_and_stays_open() {
    let model = make_composing_detail_model("");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert!(cmds.is_empty(), "an empty buffer must never submit");
    assert_eq!(next.compose.unwrap().status, ComposeStatus::Idle);
}

#[test]
fn submit_compose_with_whitespace_only_buffer_emits_no_cmd() {
    let model = make_composing_detail_model("   \n  ");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert!(
        cmds.is_empty(),
        "a whitespace-only buffer must never submit"
    );
    assert_eq!(next.compose.unwrap().status, ComposeStatus::Idle);
}

#[test]
fn submit_compose_with_no_compose_open_is_noop() {
    let model = make_compose_detail_model();

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert!(next.compose.is_none());
    assert!(cmds.is_empty());
}

// ---- S2: CommentMutationOk closes the compose and refreshes the thread ----

#[test]
fn comment_mutation_ok_closes_compose_and_emits_exactly_one_cache_busting_refresh() {
    let mut model = make_composing_detail_model("hi");
    model.compose = Some(Compose {
        buffer: "hi".to_owned(),
        status: ComposeStatus::Submitting,
        target: ComposeTarget::New,
    });

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(next.compose.is_none(), "success must close the compose");
    assert_eq!(
        cmds,
        vec![Cmd::RefreshDetail("PROJ-1".to_owned())],
        "success must emit exactly one cache-busting refresh for the open key"
    );
}

#[test]
fn comment_mutation_ok_with_no_loaded_issue_emits_no_refresh() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = None;
    model.compose = Some(Compose {
        buffer: "hi".to_owned(),
        status: ComposeStatus::Submitting,
        target: ComposeTarget::New,
    });

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(next.compose.is_none());
    assert!(cmds.is_empty());
}

// ---- S4: CommentMutationErr preserves the draft, sets Error, emits no refresh ----

#[test]
fn comment_mutation_err_preserves_buffer_sets_error_and_emits_no_refresh() {
    let mut model = make_composing_detail_model("hi");
    model.compose = Some(Compose {
        buffer: "hi".to_owned(),
        status: ComposeStatus::Submitting,
        target: ComposeTarget::New,
    });

    let (next, cmds) = update(model, Msg::CommentMutationErr("boom".to_owned()));

    let compose = next
        .compose
        .expect("a failed submit must keep the compose open");
    assert_eq!(compose.buffer, "hi", "the draft must be preserved");
    assert_eq!(compose.status, ComposeStatus::Error("boom".to_owned()));
    assert!(cmds.is_empty(), "a failed submit must emit no refresh Cmd");
}

#[test]
fn comment_mutation_err_with_no_compose_open_is_noop() {
    let model = make_compose_detail_model();

    let (next, cmds) = update(model, Msg::CommentMutationErr("boom".to_owned()));

    assert!(next.compose.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn comment_mutation_err_with_reauth_message_surfaces_the_e2_guidance() {
    let guidance = crate::commands::reauth_message("work");
    let model = make_composing_detail_model("hi");

    let (next, cmds) = update(model, Msg::CommentMutationErr(guidance.clone()));

    let compose = next.compose.expect("failure must keep the compose open");
    assert_eq!(compose.status, ComposeStatus::Error(guidance));
    assert!(cmds.is_empty());
}

// ---- S3: Esc discards the draft with no Cmd, detail untouched ----

#[test]
fn cancel_compose_closes_with_no_cmd_and_leaves_detail_untouched() {
    let model = make_composing_detail_model("draft");

    let (next, cmds) = update(model, Msg::CancelCompose);

    assert!(next.compose.is_none());
    assert!(cmds.is_empty());
    assert_eq!(
        next.detail_scroll, 2,
        "cancel must never touch detail state"
    );
}

// ---- S6: no key/mouse leakage while a compose is open ----

#[test]
fn list_and_detail_nav_keys_are_inert_while_composing() {
    fn assert_inert(msg: Msg) {
        let model = make_composing_detail_model("draft");
        let (next, cmds) = update(model, msg);

        assert!(next.compose.is_some(), "the compose must remain open");
        assert_eq!(next.screen, Screen::Detail, "screen must be unchanged");
        assert_eq!(next.detail_scroll, 2, "detail scroll must be unchanged");
        assert!(cmds.is_empty(), "a leaked msg must emit no Cmd");
    }

    assert_inert(Msg::Down);
    assert_inert(Msg::Up);
    assert_inert(Msg::Back);
    assert_inert(Msg::FocusNextLink);
    assert_inert(Msg::OpenProjects);
    assert_inert(Msg::LoadMore);
}

#[test]
fn quit_does_not_quit_while_composing() {
    let model = make_composing_detail_model("draft");

    let (next, cmds) = update(model, Msg::Quit);

    assert!(next.compose.is_some(), "compose must stay open");
    assert!(
        !cmds.contains(&Cmd::Quit),
        "q must not quit while composing"
    );
    assert!(cmds.is_empty());
}

#[test]
fn mouse_resolved_msgs_are_inert_while_composing() {
    fn assert_inert(msg: Msg) {
        let model = make_composing_detail_model("draft");
        let (next, cmds) = update(model, msg);

        assert!(next.compose.is_some(), "the compose must remain open");
        assert!(
            next.selection.is_none(),
            "no selection must be created while composing"
        );
        assert!(cmds.is_empty(), "a leaked mouse msg must emit no Cmd");
    }

    assert_inert(Msg::CardClicked(0));
    assert_inert(Msg::LinkClicked("https://example.com".to_owned()));
    assert_inert(Msg::SelStart((0, 0)));
    assert_inert(Msg::SelDrag((0, 0)));
    assert_inert(Msg::SelEnd(Some("x".to_owned())));
    assert_inert(Msg::ProjectClicked(0));
}

// ---- shell keymap: 'c' opens compose in normal mode; the compose keymap
// owns Enter/Backspace/Ctrl+S/Esc/printable chars exclusively ----

#[test]
fn map_key_in_normal_mode_c_opens_compose() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('c'), KeyModifiers::NONE),
        Some(Msg::OpenCompose)
    ));
}

#[test]
fn map_key_in_normal_mode_ctrl_c_still_quits() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(Msg::Quit)
    ));
}

#[test]
fn map_key_in_compose_mode_esc_cancels() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Esc, KeyModifiers::NONE),
        Some(Msg::CancelCompose)
    ));
}

#[test]
fn map_key_in_compose_mode_enter_inserts_newline_never_submits() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Enter, KeyModifiers::NONE),
        Some(Msg::ComposeNewline)
    ));
}

#[test]
fn map_key_in_compose_mode_backspace_deletes() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Backspace, KeyModifiers::NONE),
        Some(Msg::ComposeBackspace)
    ));
}

#[test]
fn map_key_in_compose_mode_ctrl_s_submits() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Char('s'), KeyModifiers::CONTROL),
        Some(Msg::SubmitCompose)
    ));
}

#[test]
fn map_key_in_compose_mode_plain_s_types_into_the_buffer() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Char('s'), KeyModifiers::NONE),
        Some(Msg::ComposeInput('s'))
    ));
}

#[test]
fn map_key_in_compose_mode_plain_char_appends() {
    assert!(matches!(
        map_key_in_compose_mode(KeyCode::Char('q'), KeyModifiers::NONE),
        Some(Msg::ComposeInput('q'))
    ));
}

#[test]
fn map_key_in_compose_mode_tab_is_unmapped() {
    assert!(map_key_in_compose_mode(KeyCode::Tab, KeyModifiers::NONE).is_none());
}

// ---- c4a1-comment-focus-ownership / ADR 0026 §1-§2, BDR 0017 S1-S2, S9 ----

fn make_three_comments() -> Vec<crate::models::IssueComment> {
    vec![
        comment(None, Some("Alice"), "First.", Some("2026-01-01"), None),
        comment(None, Some("Bob"), "Second.", Some("2026-01-02"), None),
        comment(None, Some("Carol"), "Third.", Some("2026-01-03"), None),
    ]
}

fn make_comment_detail_model(comments: Vec<crate::models::IssueComment>) -> Model {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;
    model.detail = Some(make_issue_with_comments("PROJ-1", comments));
    model
}

// ---- S1: focus arithmetic — ']' from None -> 0, clamps at len-1; '[' clamps
// at 0; an empty thread stays None; leaving Detail resets focus ----

#[test]
fn focus_next_comment_from_none_focuses_the_first_comment() {
    let model = make_comment_detail_model(make_three_comments());

    let (next, cmds) = update(model, Msg::FocusNextComment);

    assert_eq!(next.detail_focused_comment, Some(0));
    assert!(cmds.is_empty(), "a focus move must emit no Cmd");
}

#[test]
fn focus_next_comment_repeated_clamps_at_the_last_index() {
    let mut model = make_comment_detail_model(make_three_comments());
    model.detail_focused_comment = Some(2);

    let (next, cmds) = update(model, Msg::FocusNextComment);

    assert_eq!(
        next.detail_focused_comment,
        Some(2),
        "']' past the last comment must clamp, never wrap"
    );
    assert!(cmds.is_empty());
}

#[test]
fn focus_next_comment_advances_by_one() {
    let mut model = make_comment_detail_model(make_three_comments());
    model.detail_focused_comment = Some(0);

    let (next, cmds) = update(model, Msg::FocusNextComment);

    assert_eq!(next.detail_focused_comment, Some(1));
    assert!(cmds.is_empty());
}

#[test]
fn focus_prev_comment_from_none_focuses_the_last_comment() {
    let model = make_comment_detail_model(make_three_comments());

    let (next, cmds) = update(model, Msg::FocusPrevComment);

    assert_eq!(next.detail_focused_comment, Some(2));
    assert!(cmds.is_empty());
}

#[test]
fn focus_prev_comment_repeated_clamps_at_zero() {
    let mut model = make_comment_detail_model(make_three_comments());
    model.detail_focused_comment = Some(0);

    let (next, cmds) = update(model, Msg::FocusPrevComment);

    assert_eq!(
        next.detail_focused_comment,
        Some(0),
        "'[' before the first comment must clamp, never wrap"
    );
    assert!(cmds.is_empty());
}

#[test]
fn focus_next_comment_on_empty_thread_stays_none() {
    let model = make_comment_detail_model(vec![]);

    let (next, cmds) = update(model, Msg::FocusNextComment);

    assert_eq!(
        next.detail_focused_comment, None,
        "an empty thread must leave focus None"
    );
    assert!(cmds.is_empty());
}

#[test]
fn focus_prev_comment_on_empty_thread_stays_none() {
    let model = make_comment_detail_model(vec![]);

    let (next, cmds) = update(model, Msg::FocusPrevComment);

    assert_eq!(next.detail_focused_comment, None);
    assert!(cmds.is_empty());
}

#[test]
fn focus_next_comment_on_list_screen_is_noop() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.detail_focused_comment = Some(0);

    let (next, cmds) = update(model, Msg::FocusNextComment);

    assert_eq!(
        next.detail_focused_comment,
        Some(0),
        "FocusNextComment off the Detail screen must not change focus"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_back_from_detail_resets_focused_comment_to_none() {
    let mut model = make_comment_detail_model(make_three_comments());
    model.detail_focused_comment = Some(1);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(
        next.detail_focused_comment, None,
        "leaving the Detail screen must reset comment focus"
    );
    assert!(cmds.is_empty());
}

#[test]
fn update_detail_loaded_leaves_comment_focus_none() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.screen = Screen::Detail;

    let issue = make_issue_with_comments("PROJ-1", make_three_comments());
    let (next, _) = update(model, Msg::DetailLoaded(Box::new(issue)));

    assert_eq!(
        next.detail_focused_comment, None,
        "loading a fresh detail must start with no comment focused"
    );
}

// ---- S9: focus Msgs do not leak past an open compose ----

#[test]
fn focus_next_and_prev_comment_are_inert_while_composing() {
    fn assert_inert(msg: Msg) {
        let mut model = make_composing_detail_model("draft");
        model.detail = Some(make_issue_with_comments("PROJ-1", make_three_comments()));
        model.detail_focused_comment = Some(1);

        let (next, cmds) = update(model, msg);

        assert_eq!(
            next.detail_focused_comment,
            Some(1),
            "a focus Msg must not change focus while composing"
        );
        assert!(next.compose.is_some(), "the compose must remain open");
        assert!(cmds.is_empty());
    }

    assert_inert(Msg::FocusNextComment);
    assert_inert(Msg::FocusPrevComment);
}

// ---- S2: ownership from a one-shot myself fetch ----

#[test]
fn myself_loaded_sets_current_account_id() {
    let model = make_list_model(&["PROJ-1"]);

    let (next, cmds) = update(model, Msg::MyselfLoaded("acct-A".to_owned()));

    assert_eq!(next.current_account_id.as_deref(), Some("acct-A"));
    assert!(cmds.is_empty());
}

#[test]
fn is_own_comment_true_when_author_matches_current_account_id() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.current_account_id = Some("acct-A".to_owned());
    let own = crate::models::IssueComment {
        author_account_id: Some("acct-A".to_owned()),
        ..comment(None, Some("Alice"), "hi", None, None)
    };

    assert!(model.is_own_comment(&own));
}

#[test]
fn is_own_comment_false_when_author_does_not_match() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.current_account_id = Some("acct-A".to_owned());
    let not_own = crate::models::IssueComment {
        author_account_id: Some("acct-B".to_owned()),
        ..comment(None, Some("Bob"), "hi", None, None)
    };

    assert!(!model.is_own_comment(&not_own));
}

#[test]
fn is_own_comment_false_when_current_account_id_is_none() {
    let model = make_list_model(&["PROJ-1"]);
    let comment_by_a = crate::models::IssueComment {
        author_account_id: Some("acct-A".to_owned()),
        ..comment(None, Some("Alice"), "hi", None, None)
    };

    assert!(!model.is_own_comment(&comment_by_a));
}

#[test]
fn is_own_comment_false_when_comment_has_no_author_account_id() {
    let mut model = make_list_model(&["PROJ-1"]);
    model.current_account_id = Some("acct-A".to_owned());
    let unattributed = comment(None, Some("Alice"), "hi", None, None);

    assert!(!model.is_own_comment(&unattributed));
}

// ---- c4a2-comment-edit / ADR 0026 §3, BDR 0017 S3-S6 — 'e' opens the
// compose in edit mode for a focused OWN comment; non-own/no-focus gating;
// Ctrl+S branches on the compose target; Ok/Err reuse the C3b arms verbatim
// for the edit path ----

const OWNER_ACCOUNT_ID: &str = "acct-A";

fn own_comment_with_id(id: &str, body: &str) -> crate::models::IssueComment {
    crate::models::IssueComment {
        author_account_id: Some(OWNER_ACCOUNT_ID.to_owned()),
        ..comment(Some(id), Some("Alice"), body, None, None)
    }
}

fn comment_detail_model_with_focus(
    comments: Vec<crate::models::IssueComment>,
    focus: Option<usize>,
) -> Model {
    let mut model = make_comment_detail_model(comments);
    model.current_account_id = Some(OWNER_ACCOUNT_ID.to_owned());
    model.detail_focused_comment = focus;
    model
}

#[test]
fn edit_focused_comment_on_own_comment_opens_edit_compose_prefilled_with_body_and_title() {
    let own = own_comment_with_id("10001", "hello world");
    let model = comment_detail_model_with_focus(vec![own], Some(0));

    let (next, cmds) = update(model, Msg::EditFocusedComment);

    let compose = next
        .compose
        .expect("'e' on an own comment with an id must open the compose");
    assert_eq!(
        compose.target,
        ComposeTarget::Edit {
            comment_id: "10001".to_owned()
        },
        "the compose target must carry the focused comment's id"
    );
    assert_eq!(
        compose.buffer, "hello world",
        "the buffer must equal adf_to_plain_text(comment.body)"
    );
    assert_eq!(compose.status, ComposeStatus::Idle);
    assert_eq!(
        compose.target.title_key(),
        "Edit comment",
        "the title-key helper must resolve to Edit comment"
    );
    assert!(cmds.is_empty(), "opening the edit compose must emit no Cmd");
}

#[test]
fn edit_focused_comment_on_non_own_comment_sets_hint_and_opens_no_compose() {
    let not_own = crate::models::IssueComment {
        author_account_id: Some("acct-B".to_owned()),
        ..comment(Some("10002"), Some("Bob"), "hi", None, None)
    };
    let model = comment_detail_model_with_focus(vec![not_own], Some(0));

    let (next, cmds) = update(model, Msg::EditFocusedComment);

    assert!(
        next.compose.is_none(),
        "'e' on a non-own comment must open no compose"
    );
    let status = next
        .status
        .expect("'e' on a non-own comment must set a status hint");
    assert_eq!(status.kind, StatusKind::Info);
    assert!(!status.text.is_empty(), "the hint text must not be empty");
    assert!(cmds.is_empty(), "'e' on a non-own comment must emit no Cmd");
}

#[test]
fn edit_focused_comment_with_no_focus_is_noop() {
    let own = own_comment_with_id("10001", "hello");
    let model = comment_detail_model_with_focus(vec![own], None);

    let (next, cmds) = update(model, Msg::EditFocusedComment);

    assert!(
        next.compose.is_none(),
        "no focused comment must open no compose"
    );
    assert!(
        next.status.is_none(),
        "no focused comment must change no status"
    );
    assert!(cmds.is_empty());
}

#[test]
fn edit_focused_comment_on_own_comment_with_no_id_is_noop() {
    let own_no_id = crate::models::IssueComment {
        author_account_id: Some(OWNER_ACCOUNT_ID.to_owned()),
        ..comment(None, Some("Alice"), "hello", None, None)
    };
    let model = comment_detail_model_with_focus(vec![own_no_id], Some(0));

    let (next, cmds) = update(model, Msg::EditFocusedComment);

    assert!(
        next.compose.is_none(),
        "an own comment with no id must open no compose"
    );
    assert!(next.status.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn submit_compose_edit_target_emits_exactly_one_edit_comment_and_sets_submitting() {
    let model = make_editing_detail_model("updated text", "10001");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert_eq!(
        cmds,
        vec![Cmd::EditComment {
            key: "PROJ-1".to_owned(),
            comment_id: "10001".to_owned(),
            body: "updated text".to_owned(),
        }],
        "a non-empty Edit-target buffer must emit exactly one EditComment"
    );
    let compose = next
        .compose
        .expect("the compose must stay open while submitting");
    assert_eq!(compose.status, ComposeStatus::Submitting);
    assert_eq!(compose.buffer, "updated text");
}

#[test]
fn submit_compose_edit_target_with_empty_buffer_emits_no_cmd() {
    let model = make_editing_detail_model("   \n  ", "10001");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert!(
        cmds.is_empty(),
        "an empty/whitespace Edit buffer must never submit"
    );
    assert_eq!(next.compose.unwrap().status, ComposeStatus::Idle);
}

#[test]
fn submit_compose_new_target_still_emits_submit_comment_not_edit_comment() {
    let model = make_composing_detail_model("hi");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert_eq!(
        cmds,
        vec![Cmd::SubmitComment {
            key: "PROJ-1".to_owned(),
            body: "hi".to_owned(),
        }],
        "a New-target compose must still emit SubmitComment (regression guard)"
    );
    assert_eq!(next.compose.unwrap().status, ComposeStatus::Submitting);
}

#[test]
fn comment_mutation_ok_after_edit_closes_compose_and_emits_one_refresh() {
    let mut model = make_editing_detail_model("updated", "10001");
    model.compose = Some(Compose {
        status: ComposeStatus::Submitting,
        ..model.compose.unwrap()
    });

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(
        next.compose.is_none(),
        "an edit success must close the compose"
    );
    assert_eq!(
        cmds,
        vec![Cmd::RefreshDetail("PROJ-1".to_owned())],
        "an edit success must emit exactly one cache-busting refresh, no local mutation"
    );
}

#[test]
fn edit_focused_comment_is_inert_while_composing() {
    let mut model = make_composing_detail_model("draft");
    model.detail = Some(make_issue_with_comments(
        "PROJ-1",
        vec![own_comment_with_id("10001", "hello")],
    ));
    model.current_account_id = Some(OWNER_ACCOUNT_ID.to_owned());
    model.detail_focused_comment = Some(0);

    let (next, cmds) = update(model, Msg::EditFocusedComment);

    assert_eq!(
        next.compose.unwrap().buffer,
        "draft",
        "EditFocusedComment must not replace the open compose while composing"
    );
    assert!(cmds.is_empty());
}

#[test]
fn comment_mutation_err_after_edit_preserves_buffer_and_sets_reauth_error() {
    let mut model = make_editing_detail_model("updated", "10001");
    model.compose = Some(Compose {
        status: ComposeStatus::Submitting,
        ..model.compose.unwrap()
    });
    let guidance = crate::commands::reauth_message("work");

    let (next, cmds) = update(model, Msg::CommentMutationErr(guidance.clone()));

    let compose = next
        .compose
        .expect("an edit failure must keep the compose open");
    assert_eq!(compose.buffer, "updated", "the draft must be preserved");
    assert_eq!(compose.status, ComposeStatus::Error(guidance));
    assert!(
        cmds.is_empty(),
        "an edit failure must emit zero refresh Cmds"
    );
}

// ---- c4b-delete-confirm-modal / ADR 0026 §4, BDR 0017 S6-S7, S9-S10 —
// 'd' opens a Sim/Não confirm for a focused OWN comment; non-own/no-focus
// gating mirrors 'e'; Yes emits exactly one DeleteComment; No/Ok/Err close
// the confirm; Ok/Err become context-aware between confirm and compose ----

fn make_confirming_detail_model(comment_id: &str) -> Model {
    let mut model = make_compose_detail_model();
    model.confirm = Some(ConfirmDelete {
        comment_id: comment_id.to_owned(),
    });
    model
}

// ---- S6-S7: 'd' gating mirrors 'e' — own+id opens the confirm; non-own
// sets the hint; no focus / own-without-id is a no-op ----

#[test]
fn delete_focused_comment_on_own_comment_with_id_opens_confirm() {
    let own = own_comment_with_id("10001", "hello world");
    let model = comment_detail_model_with_focus(vec![own], Some(0));

    let (next, cmds) = update(model, Msg::DeleteFocusedComment);

    assert_eq!(
        next.confirm,
        Some(ConfirmDelete {
            comment_id: "10001".to_owned()
        }),
        "'d' on an own comment with an id must open the confirm carrying its id"
    );
    assert!(cmds.is_empty(), "opening the confirm must emit no Cmd");
}

#[test]
fn delete_focused_comment_on_non_own_comment_sets_hint_and_opens_no_confirm() {
    let not_own = crate::models::IssueComment {
        author_account_id: Some("acct-B".to_owned()),
        ..comment(Some("10002"), Some("Bob"), "hi", None, None)
    };
    let model = comment_detail_model_with_focus(vec![not_own], Some(0));

    let (next, cmds) = update(model, Msg::DeleteFocusedComment);

    assert!(
        next.confirm.is_none(),
        "'d' on a non-own comment must open no confirm"
    );
    let status = next
        .status
        .expect("'d' on a non-own comment must set a status hint");
    assert_eq!(status.kind, StatusKind::Info);
    assert!(!status.text.is_empty(), "the hint text must not be empty");
    assert!(cmds.is_empty(), "'d' on a non-own comment must emit no Cmd");
}

#[test]
fn delete_focused_comment_with_no_focus_is_noop() {
    let own = own_comment_with_id("10001", "hello");
    let model = comment_detail_model_with_focus(vec![own], None);

    let (next, cmds) = update(model, Msg::DeleteFocusedComment);

    assert!(
        next.confirm.is_none(),
        "no focused comment must open no confirm"
    );
    assert!(
        next.status.is_none(),
        "no focused comment must change no status"
    );
    assert!(cmds.is_empty());
}

#[test]
fn delete_focused_comment_on_own_comment_with_no_id_is_noop() {
    let own_no_id = crate::models::IssueComment {
        author_account_id: Some(OWNER_ACCOUNT_ID.to_owned()),
        ..comment(None, Some("Alice"), "hello", None, None)
    };
    let model = comment_detail_model_with_focus(vec![own_no_id], Some(0));

    let (next, cmds) = update(model, Msg::DeleteFocusedComment);

    assert!(
        next.confirm.is_none(),
        "an own comment with no id must open no confirm"
    );
    assert!(next.status.is_none());
    assert!(cmds.is_empty());
}

// ---- S7: Yes emits exactly one DeleteComment; No closes with no Cmd ----

#[test]
fn confirm_delete_yes_emits_exactly_one_delete_comment_cmd() {
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::ConfirmDeleteYes);

    assert_eq!(
        cmds,
        vec![Cmd::DeleteComment {
            key: "PROJ-1".to_owned(),
            comment_id: "10001".to_owned(),
        }],
        "Yes must emit exactly one DeleteComment for the open key and comment id"
    );
    assert!(
        next.confirm.is_some(),
        "the confirm stays open as the in-flight indicator until the mutation result closes it"
    );
}

#[test]
fn confirm_delete_yes_with_no_confirm_open_is_noop() {
    let model = make_compose_detail_model();

    let (next, cmds) = update(model, Msg::ConfirmDeleteYes);

    assert!(next.confirm.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn confirm_delete_no_closes_confirm_with_no_cmd() {
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::ConfirmDeleteNo);

    assert!(next.confirm.is_none(), "No must close the confirm");
    assert!(cmds.is_empty(), "No must emit no Cmd");
}

// ---- S7: CommentMutationOk/Err become context-aware for a delete in
// flight; the existing compose Ok/Err arms stay byte-for-byte unchanged
// (regression, exercised above) ----

#[test]
fn comment_mutation_ok_with_confirm_open_closes_confirm_and_emits_one_refresh() {
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(
        next.confirm.is_none(),
        "a delete success must close the confirm"
    );
    assert_eq!(
        cmds,
        vec![Cmd::RefreshDetail("PROJ-1".to_owned())],
        "a delete success must emit exactly one cache-busting refresh, no local removal"
    );
}

#[test]
fn comment_mutation_ok_with_compose_open_only_closes_compose_confirm_stays_none() {
    let mut model = make_composing_detail_model("hi");
    model.compose = Some(Compose {
        buffer: "hi".to_owned(),
        status: ComposeStatus::Submitting,
        target: ComposeTarget::New,
    });

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(next.compose.is_none());
    assert!(
        next.confirm.is_none(),
        "a compose success must never touch confirm (it was already None)"
    );
    assert_eq!(cmds, vec![Cmd::RefreshDetail("PROJ-1".to_owned())]);
}

#[test]
fn comment_mutation_err_with_confirm_open_closes_confirm_and_sets_error_status() {
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::CommentMutationErr("boom".to_owned()));

    assert!(
        next.confirm.is_none(),
        "a delete failure must close the confirm"
    );
    let status = next
        .status
        .expect("a delete failure must set a transient status");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, "boom");
    assert!(
        cmds.is_empty(),
        "a delete failure must emit zero refresh Cmds"
    );
}

#[test]
fn comment_mutation_err_with_confirm_open_surfaces_reauth_guidance() {
    let guidance = crate::commands::reauth_message("work");
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::CommentMutationErr(guidance.clone()));

    assert!(next.confirm.is_none());
    let status = next
        .status
        .expect("a delete failure must set a transient status");
    assert_eq!(status.kind, StatusKind::Error);
    assert_eq!(status.text, guidance);
    assert!(cmds.is_empty());
}

// ---- S9: no key/mouse leakage while a delete confirm is open ----

#[test]
fn list_and_detail_nav_keys_are_inert_while_confirm_open() {
    fn assert_inert(msg: Msg) {
        let model = make_confirming_detail_model("10001");
        let (next, cmds) = update(model, msg);

        assert!(next.confirm.is_some(), "the confirm must remain open");
        assert_eq!(next.screen, Screen::Detail, "screen must be unchanged");
        assert!(cmds.is_empty(), "a leaked msg must emit no Cmd");
    }

    assert_inert(Msg::Down);
    assert_inert(Msg::Up);
    assert_inert(Msg::Back);
    assert_inert(Msg::FocusNextLink);
    assert_inert(Msg::FocusNextComment);
    assert_inert(Msg::FocusPrevComment);
    assert_inert(Msg::OpenSearch);
    assert_inert(Msg::OpenProjects);
    assert_inert(Msg::LoadMore);
    assert_inert(Msg::EditFocusedComment);
    assert_inert(Msg::ReplyToFocusedComment);
}

#[test]
fn quit_does_not_quit_while_confirm_open() {
    let model = make_confirming_detail_model("10001");

    let (next, cmds) = update(model, Msg::Quit);

    assert!(next.confirm.is_some(), "confirm must stay open");
    assert!(
        !cmds.contains(&Cmd::Quit),
        "q must not quit while confirming"
    );
    assert!(cmds.is_empty());
}

#[test]
fn mouse_resolved_msgs_are_inert_while_confirm_open() {
    fn assert_inert(msg: Msg) {
        let model = make_confirming_detail_model("10001");
        let (next, cmds) = update(model, msg);

        assert!(next.confirm.is_some(), "the confirm must remain open");
        assert!(cmds.is_empty(), "a leaked mouse msg must emit no Cmd");
    }

    assert_inert(Msg::CardClicked(0));
    assert_inert(Msg::LinkClicked("https://example.com".to_owned()));
    assert_inert(Msg::SelStart((0, 0)));
    assert_inert(Msg::SelDrag((0, 0)));
    assert_inert(Msg::SelEnd(Some("x".to_owned())));
    assert_inert(Msg::ProjectClicked(0));
}

// ---- S10: the pure confirm_modal_content builder ----

#[test]
fn confirm_modal_content_yields_localized_title_prompt_and_buttons() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let content = view::confirm_modal_content();
    assert_eq!(content.title, "Delete comment?");
    assert_eq!(
        content.body.len(),
        1,
        "the confirm prompt must be a single body line"
    );
    let labels: Vec<String> = content.buttons.iter().map(|b| b.label.clone()).collect();
    assert_eq!(labels, vec!["Yes".to_owned(), "No".to_owned()]);

    set_language("pt_BR");
    let content_pt = view::confirm_modal_content();
    assert_eq!(content_pt.title, "Excluir comentário?");
    let labels_pt: Vec<String> = content_pt.buttons.iter().map(|b| b.label.clone()).collect();
    assert_eq!(labels_pt, vec!["Sim".to_owned(), "Não".to_owned()]);

    set_language("en");
}

// ---- shell keymap: 'd' opens the delete confirm; the confirm keymap owns
// y/Enter/n/Esc exclusively ----

#[test]
fn map_key_in_normal_mode_d_opens_delete_confirm() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('d'), KeyModifiers::NONE),
        Some(Msg::DeleteFocusedComment)
    ));
}

#[test]
fn map_key_in_confirm_mode_enter_confirms() {
    assert!(matches!(
        map_key_in_confirm_mode(KeyCode::Enter, KeyModifiers::NONE),
        Some(Msg::ConfirmDeleteYes)
    ));
}

#[test]
fn map_key_in_confirm_mode_y_confirms() {
    assert!(matches!(
        map_key_in_confirm_mode(KeyCode::Char('y'), KeyModifiers::NONE),
        Some(Msg::ConfirmDeleteYes)
    ));
}

#[test]
fn map_key_in_confirm_mode_esc_cancels() {
    assert!(matches!(
        map_key_in_confirm_mode(KeyCode::Esc, KeyModifiers::NONE),
        Some(Msg::ConfirmDeleteNo)
    ));
}

#[test]
fn map_key_in_confirm_mode_n_cancels() {
    assert!(matches!(
        map_key_in_confirm_mode(KeyCode::Char('n'), KeyModifiers::NONE),
        Some(Msg::ConfirmDeleteNo)
    ));
}

#[test]
fn map_key_in_confirm_mode_other_keys_are_noop() {
    assert!(map_key_in_confirm_mode(KeyCode::Tab, KeyModifiers::NONE).is_none());
    assert!(map_key_in_confirm_mode(KeyCode::Char('q'), KeyModifiers::NONE).is_none());
}

// ---- c4c-reply-mention / ADR 0026 §5, BDR 0017 S8 — 'r' opens the compose
// to post a NEW comment carrying a structural mention of the focused
// comment's author; NOT ownership-gated (unlike 'e'/'d'); submit emits
// exactly one Cmd::ReplyComment; Ok/Err reuse the C3b/C4a arms verbatim ----

fn make_replying_detail_model(
    buffer: &str,
    mention_account_id: &str,
    mention_display: &str,
) -> Model {
    let mut model = make_compose_detail_model();
    model.detail_scroll = 2;
    model.compose = Some(Compose {
        buffer: buffer.to_owned(),
        status: ComposeStatus::Idle,
        target: ComposeTarget::Reply {
            mention_account_id: mention_account_id.to_owned(),
            mention_display: mention_display.to_owned(),
        },
    });
    model
}

#[test]
fn reply_to_focused_comment_opens_new_compose_seeded_with_mention_from_author() {
    let author = crate::models::IssueComment {
        author_account_id: Some("acct-B".to_owned()),
        ..comment(Some("10002"), Some("Bob"), "hi", None, None)
    };
    let model = comment_detail_model_with_focus(vec![author], Some(0));

    let (next, cmds) = update(model, Msg::ReplyToFocusedComment);

    let compose = next
        .compose
        .expect("'r' on a focused comment must open the compose");
    assert_eq!(
        compose.target,
        ComposeTarget::Reply {
            mention_account_id: "acct-B".to_owned(),
            mention_display: "Bob".to_owned(),
        },
        "the compose target must carry the focused comment's author"
    );
    assert_eq!(
        compose.buffer, "",
        "the buffer must start EMPTY — the mention is carried structurally, never seeded"
    );
    assert_eq!(compose.status, ComposeStatus::Idle);
    assert_eq!(
        compose.target.title_key(),
        "Reply comment",
        "the title-key helper must resolve to Reply comment"
    );
    assert!(
        cmds.is_empty(),
        "opening the reply compose must emit no Cmd"
    );
}

#[test]
fn reply_to_focused_own_comment_still_opens_compose_not_gated() {
    let own = own_comment_with_id("10001", "hello world");
    let model = comment_detail_model_with_focus(vec![own], Some(0));

    let (next, cmds) = update(model, Msg::ReplyToFocusedComment);

    let compose = next
        .compose
        .expect("'r' on the user's own comment must still open the compose");
    assert_eq!(
        compose.target,
        ComposeTarget::Reply {
            mention_account_id: OWNER_ACCOUNT_ID.to_owned(),
            mention_display: "Alice".to_owned(),
        }
    );
    assert!(
        next.status.is_none(),
        "reply must never set the 'not your comment' hint — it is not ownership-gated"
    );
    assert!(cmds.is_empty());
}

#[test]
fn reply_to_focused_comment_with_no_focus_is_noop() {
    let own = own_comment_with_id("10001", "hello");
    let model = comment_detail_model_with_focus(vec![own], None);

    let (next, cmds) = update(model, Msg::ReplyToFocusedComment);

    assert!(
        next.compose.is_none(),
        "no focused comment must open no compose"
    );
    assert!(cmds.is_empty());
}

#[test]
fn reply_to_focused_comment_with_no_author_account_id_is_noop() {
    let unattributed = comment(Some("10003"), Some("Nobody"), "hi", None, None);
    let model = comment_detail_model_with_focus(vec![unattributed], Some(0));

    let (next, cmds) = update(model, Msg::ReplyToFocusedComment);

    assert!(
        next.compose.is_none(),
        "a comment with no author_account_id must open no compose"
    );
    assert!(cmds.is_empty());
}

#[test]
fn reply_to_focused_comment_is_inert_while_composing() {
    let mut model = make_composing_detail_model("draft");
    model.detail = Some(make_issue_with_comments(
        "PROJ-1",
        vec![own_comment_with_id("10001", "hello")],
    ));
    model.detail_focused_comment = Some(0);

    let (next, cmds) = update(model, Msg::ReplyToFocusedComment);

    assert_eq!(
        next.compose.unwrap().buffer,
        "draft",
        "ReplyToFocusedComment must not replace the open compose while composing"
    );
    assert!(cmds.is_empty());
}

// ---- S8: SubmitCompose on a Reply target emits exactly one ReplyComment
// carrying the mention fields + trimmed body; empty/whitespace emits none ----

#[test]
fn submit_compose_reply_target_emits_exactly_one_reply_comment_and_sets_submitting() {
    let model = make_replying_detail_model("hi there", "acct-B", "Bob");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert_eq!(
        cmds,
        vec![Cmd::ReplyComment {
            key: "PROJ-1".to_owned(),
            mention_account_id: "acct-B".to_owned(),
            mention_display: "Bob".to_owned(),
            body: "hi there".to_owned(),
        }],
        "a non-empty Reply-target buffer must emit exactly one ReplyComment"
    );
    let compose = next
        .compose
        .expect("the compose must stay open while submitting");
    assert_eq!(compose.status, ComposeStatus::Submitting);
    assert_eq!(compose.buffer, "hi there");
}

#[test]
fn submit_compose_reply_target_with_empty_buffer_emits_no_cmd() {
    let model = make_replying_detail_model("   \n  ", "acct-B", "Bob");

    let (next, cmds) = update(model, Msg::SubmitCompose);

    assert!(
        cmds.is_empty(),
        "an empty/whitespace Reply buffer must never submit"
    );
    assert_eq!(next.compose.unwrap().status, ComposeStatus::Idle);
}

// ---- S8: CommentMutationOk/Err reuse the C3b/C4a arms verbatim for a Reply
// compose ----

#[test]
fn comment_mutation_ok_after_reply_closes_compose_and_emits_one_refresh() {
    let mut model = make_replying_detail_model("hi", "acct-B", "Bob");
    model.compose = Some(Compose {
        status: ComposeStatus::Submitting,
        ..model.compose.unwrap()
    });

    let (next, cmds) = update(model, Msg::CommentMutationOk);

    assert!(
        next.compose.is_none(),
        "a reply success must close the compose"
    );
    assert_eq!(
        cmds,
        vec![Cmd::RefreshDetail("PROJ-1".to_owned())],
        "a reply success must emit exactly one cache-busting refresh, no local insertion"
    );
}

#[test]
fn comment_mutation_err_after_reply_preserves_buffer_and_sets_reauth_error() {
    let mut model = make_replying_detail_model("hi", "acct-B", "Bob");
    model.compose = Some(Compose {
        status: ComposeStatus::Submitting,
        ..model.compose.unwrap()
    });
    let guidance = crate::commands::reauth_message("work");

    let (next, cmds) = update(model, Msg::CommentMutationErr(guidance.clone()));

    let compose = next
        .compose
        .expect("a reply failure must keep the compose open");
    assert_eq!(compose.buffer, "hi", "the draft must be preserved");
    assert_eq!(compose.status, ComposeStatus::Error(guidance));
    assert!(
        cmds.is_empty(),
        "a reply failure must emit zero refresh Cmds"
    );
}

// ---- shell keymap: 'r' opens the reply compose ----

#[test]
fn map_key_in_normal_mode_r_opens_reply_to_focused_comment() {
    assert!(matches!(
        map_key_in_normal_mode(KeyCode::Char('r'), KeyModifiers::NONE),
        Some(Msg::ReplyToFocusedComment)
    ));
}
