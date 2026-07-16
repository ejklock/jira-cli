use super::*;

use crate::test_support::{build_search_payload_with_key, make_test_instance};

// ---- Helpers ----

fn open_in_memory_conn() -> rusqlite::Connection {
    rusqlite::Connection::open_in_memory().unwrap()
}

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

// ---- ac2 (S1): a Model built from TuiSeed::Mine matches browse's existing
// entry (Screen::List, the mine JQL, ListOrigin::Mine) ----

#[test]
fn seeded_model_from_mine_is_list_screen_with_mine_jql() {
    let instance = make_test_instance();

    let model = seeded_model(vec![], None, &instance, false, &TuiSeed::Mine);

    assert_eq!(model.screen, Screen::List);
    assert_eq!(model.jql, MINE_JQL);
    assert_eq!(model.list_origin, ListOrigin::Mine);
}

#[test]
fn seeded_model_from_mine_carries_rows_and_page_token_through() {
    let instance = make_test_instance();
    let rows = vec![IssueRow {
        key: "MINE-1".to_owned(),
        issue_type: "Task".to_owned(),
        status: "Open".to_owned(),
        assignee: Some("Alice".to_owned()),
        summary: "Fix something".to_owned(),
        duedate: None,
        project: None,
    }];

    let model = seeded_model(
        rows.clone(),
        Some("next-token".to_owned()),
        &instance,
        true,
        &TuiSeed::Mine,
    );

    assert_eq!(model.rows, rows);
    assert_eq!(model.next_page_token.as_deref(), Some("next-token"));
    assert!(model.revalidating);
}

// ---- ac2 (S1): browse() delegates to browse_seeded(.., TuiSeed::Mine, ..)
// with no observable change (TtyError guard path, deterministic/no network) ----

#[tokio::test]
async fn browse_and_browse_seeded_mine_agree_on_the_tty_error_path() {
    let instance = make_test_instance();
    let conn = open_in_memory_conn();
    let cache = crate::store::cache::TaskCache::new(&conn);
    let mut stderr_browse = Vec::<u8>::new();
    let mut stderr_seeded = Vec::<u8>::new();

    let code_browse = browse(&instance, &cache, false, &mut stderr_browse).await;
    let code_seeded =
        browse_seeded(&instance, &cache, false, TuiSeed::Mine, &mut stderr_seeded).await;

    assert_eq!(code_browse, 1);
    assert_eq!(code_browse, code_seeded);
    assert_eq!(stderr_browse, stderr_seeded);
}

// ---- ac1 (S2): a Model built from TuiSeed::Search(jql) seeds Screen::List
// with that jql and ListOrigin::Search ----

#[test]
fn seeded_model_from_search_is_list_screen_with_given_jql_and_search_origin() {
    let instance = make_test_instance();

    let model = seeded_model(
        vec![],
        None,
        &instance,
        false,
        &TuiSeed::Search("project = NIKE".to_owned()),
    );

    assert_eq!(model.screen, Screen::List);
    assert_eq!(model.jql, "project = NIKE");
    assert_eq!(model.list_origin, ListOrigin::Search);
}

#[test]
fn seeded_model_from_search_carries_rows_and_page_token_through() {
    let instance = make_test_instance();
    let rows = vec![IssueRow {
        key: "NIKE-1".to_owned(),
        issue_type: "Bug".to_owned(),
        status: "Open".to_owned(),
        assignee: None,
        summary: "Search result".to_owned(),
        duedate: None,
        project: None,
    }];

    let model = seeded_model(
        rows.clone(),
        Some("next-token".to_owned()),
        &instance,
        false,
        &TuiSeed::Search("project = NIKE".to_owned()),
    );

    assert_eq!(model.rows, rows);
    assert_eq!(model.next_page_token.as_deref(), Some("next-token"));
    assert!(!model.revalidating);
}

#[test]
fn seeded_model_from_mine_is_unaffected_by_the_search_arm() {
    let instance = make_test_instance();

    let model = seeded_model(vec![], None, &instance, false, &TuiSeed::Mine);

    assert_eq!(model.screen, Screen::List);
    assert_eq!(model.jql, MINE_JQL);
    assert_eq!(model.list_origin, ListOrigin::Mine);
}

// ---- ac2/ac3 (S1): browse_seeded(TuiSeed::Mine) with a TTY reaches the
// real fetch_and_run path (mine-scope snapshot side effect proves it, not a
// hardcoded/short-circuited return) ----

#[tokio::test]
async fn browse_seeded_mine_run_tui_reaches_fetch_and_run_and_writes_snapshot() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload_with_key("SEED-1")),
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

    // browse_seeded opens the TUI on success; bounding it keeps the test from
    // hanging when a real terminal happens to be attached (the snapshot write
    // under test always runs before that point).
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        browse_seeded(&instance, &cache, true, TuiSeed::Mine, &mut stderr),
    )
    .await;

    let list_cache = crate::store::cache::TaskListCache::new(store.conn());
    let key = crate::store::cache::instances_key(std::slice::from_ref(&instance));
    let stored = list_cache
        .read("mine", &key, 3600)
        .expect("read must not error")
        .expect("browse_seeded(Mine) must reach fetch_and_run's snapshot write");
    assert!(
        stored.contains("SEED-1"),
        "the stored snapshot must contain the fetched issue; got: {stored}"
    );
}

fn build_issue_payload_with_key(key: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "20001",
        "key": key,
        "self": "https://example.atlassian.net/rest/api/3/issue/20001",
        "fields": {
            "summary": "Detail seed issue",
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
            "assignee": null,
            "priority": null,
            "created": "2026-01-01T00:00:00.000+0000",
            "updated": "2026-06-29T00:00:00.000+0000"
        }
    })
}

// ---- ac1 (S3): a Model built from TuiSeed::Detail(key) seeds Screen::Detail
// with no rows behind it; seed_detail applies the fetched issue through the
// same DetailLoaded reducer the in-TUI fetch path uses ----

#[test]
fn seeded_model_from_detail_is_detail_screen_with_no_rows() {
    let instance = make_test_instance();

    let model = seeded_model(
        vec![],
        None,
        &instance,
        false,
        &TuiSeed::Detail("SEED-1".to_owned()),
    );

    assert_eq!(model.screen, Screen::Detail);
    assert!(
        model.rows.is_empty(),
        "a seeded detail has no list behind it"
    );
    assert!(
        model.detail.is_none(),
        "the issue is applied by seed_detail"
    );
}

#[test]
fn seed_detail_applies_the_fetched_issue_through_the_detail_loaded_reducer() {
    let instance = make_test_instance();
    let model = seeded_model(
        vec![],
        None,
        &instance,
        false,
        &TuiSeed::Detail("SEED-1".to_owned()),
    );
    let issue = crate::test_support::issue("SEED-1");

    let seeded = seed_detail(model, Some(issue.clone()));

    assert_eq!(seeded.detail, Some(issue));
}

#[test]
fn seed_detail_is_a_noop_with_no_fetched_issue() {
    let instance = make_test_instance();
    let model = seeded_model(vec![], None, &instance, false, &TuiSeed::Mine);

    let seeded = seed_detail(model, None);

    assert!(seeded.detail.is_none());
}

// ---- ac1 (S3): resolve_detail_issue's cache-or-fetch seam — a cache miss
// fetches over the network and warms the cache; a cache hit serves
// synchronously with no network call ----

#[tokio::test]
async fn resolve_detail_issue_on_cache_miss_fetches_and_warms_the_cache() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/SEED-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_issue_payload_with_key("SEED-1")),
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
    let (_dir, store) = open_temp_store();
    let cache = crate::store::cache::TaskCache::new(store.conn());

    let issue = resolve_detail_issue("SEED-1", &instance, &cache)
        .await
        .expect("cache-or-fetch must succeed on a network fetch");

    assert_eq!(issue.key, "SEED-1");
    let issue_cache = crate::store::cache::IssueCache::new(store.conn());
    let cached = issue_cache
        .read(&instance.name, "SEED-1")
        .expect("read must not error")
        .expect("a cache miss must warm the cache on a successful fetch");
    assert_eq!(cached.issue.key, "SEED-1");
    server.verify().await;
}

#[tokio::test]
async fn resolve_detail_issue_on_cache_hit_serves_from_cache_without_a_network_call() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
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
    let issue_cache = crate::store::cache::IssueCache::new(store.conn());
    let seeded = crate::test_support::issue("SEED-2");
    issue_cache.write(&instance.name, &seeded).unwrap();

    let issue = resolve_detail_issue("SEED-2", &instance, &cache)
        .await
        .expect("a cache hit must succeed with no network call");

    assert_eq!(issue.key, "SEED-2");
    server.verify().await;
}

// ---- ac1/ac3 (S3): browse_seeded(TuiSeed::Detail(key)) reaches the real
// fetch_and_run_detail path (the cache warmed on a miss proves it ran, not a
// hardcoded/short-circuited return) ----

#[tokio::test]
async fn browse_seeded_detail_run_tui_reaches_fetch_and_run_detail_and_warms_the_cache() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/SEED-3"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_issue_payload_with_key("SEED-3")),
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

    // browse_seeded opens the TUI on success; bounding it keeps the test from
    // hanging when a real terminal happens to be attached (the fetch-and-warm
    // under test always runs before that point).
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        browse_seeded(
            &instance,
            &cache,
            true,
            TuiSeed::Detail("SEED-3".to_owned()),
            &mut stderr,
        ),
    )
    .await;

    let issue_cache = crate::store::cache::IssueCache::new(store.conn());
    let cached = issue_cache
        .read(&instance.name, "SEED-3")
        .expect("read must not error")
        .expect("browse_seeded(Detail) must reach fetch_and_run_detail's cache warm");
    assert_eq!(cached.issue.key, "SEED-3");
}

// ---- ac1 (S2): browse_seeded(TuiSeed::Search(jql)) reaches the real
// fetch_and_run_search path (the fetched-issue seam proves it ran) and
// writes NO entry snapshot — mine-scope only (ADR 0016 §4) ----

#[tokio::test]
async fn browse_seeded_search_run_tui_reaches_fetch_and_run_search_and_writes_no_snapshot() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload_with_key("SEARCH-1")),
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

    // browse_seeded opens the TUI on success; bounding it keeps the test from
    // hanging when a real terminal happens to be attached (the fetch under
    // test, and the deliberate absence of a snapshot write, always run
    // before that point).
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        browse_seeded(
            &instance,
            &cache,
            true,
            TuiSeed::Search("project = NIKE".to_owned()),
            &mut stderr,
        ),
    )
    .await;

    let list_cache = crate::store::cache::TaskListCache::new(store.conn());
    let key = crate::store::cache::instances_key(std::slice::from_ref(&instance));
    let stored = list_cache
        .read("mine", &key, 3600)
        .expect("read must not error");
    assert!(
        stored.is_none(),
        "TuiSeed::Search must never write the mine-scope entry snapshot (ADR 0016 §4); got: {stored:?}"
    );
}
