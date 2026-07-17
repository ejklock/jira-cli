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

// ---- c3b-comment-compose / ADR 0024 §4 / BDR 0015 S2, S4 — submit_comment's
// reply mapping: 2xx -> Ok, Unauthorized (401) -> the typed ClientError the
// spawn wrapper turns into the same E2 re-auth guidance every other write
// seam uses ----

fn build_comment_payload_with_id(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "self": "https://example.atlassian.net/rest/api/3/issue/PROJ-1/comment/1",
    })
}

#[tokio::test]
async fn submit_comment_2xx_is_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(build_comment_payload_with_id("1")))
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

    let result = submit_comment(&instance, "PROJ-1", "hello").await;

    assert!(result.is_ok(), "a 2xx add_comment response must be Ok");
    server.verify().await;
}

#[tokio::test]
async fn submit_comment_401_maps_to_typed_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = submit_comment(&instance, "PROJ-1", "hello").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_submit_comment_2xx_replies_comment_mutation_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(build_comment_payload_with_id("1")))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_submit_comment("PROJ-1".to_owned(), "hello".to_owned(), instance, tx);

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(matches!(reply, Msg::CommentMutationOk));
}

#[tokio::test]
async fn spawn_submit_comment_401_replies_comment_mutation_err_with_reauth_guidance() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_submit_comment("PROJ-1".to_owned(), "hello".to_owned(), instance, tx);

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    match reply {
        Msg::CommentMutationErr(reason) => {
            assert_eq!(reason, reauth_message("test"));
        }
        _ => panic!("expected Msg::CommentMutationErr, got a different Msg"),
    }
}

// ---- c4a2-comment-edit / ADR 0026 §3 / BDR 0017 S4-S5 — edit_comment's reply
// mapping mirrors submit_comment's: 2xx -> Ok, Unauthorized (401) -> the
// typed ClientError the spawn wrapper turns into the E2 re-auth guidance ----

#[tokio::test]
async fn edit_comment_2xx_is_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_comment_payload_with_id("10001")),
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

    let result = edit_comment(&instance, "PROJ-1", "10001", "updated").await;

    assert!(result.is_ok(), "a 2xx update_comment response must be Ok");
    server.verify().await;
}

#[tokio::test]
async fn edit_comment_401_maps_to_typed_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = edit_comment(&instance, "PROJ-1", "10001", "updated").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_edit_comment_2xx_replies_comment_mutation_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_comment_payload_with_id("10001")),
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
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_edit_comment(
        "PROJ-1".to_owned(),
        "10001".to_owned(),
        "updated".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(matches!(reply, Msg::CommentMutationOk));
}

#[tokio::test]
async fn spawn_edit_comment_401_replies_comment_mutation_err_with_reauth_guidance() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_edit_comment(
        "PROJ-1".to_owned(),
        "10001".to_owned(),
        "updated".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    match reply {
        Msg::CommentMutationErr(reason) => {
            assert_eq!(reason, reauth_message("test"));
        }
        _ => panic!("expected Msg::CommentMutationErr, got a different Msg"),
    }
}

#[tokio::test]
async fn edit_comment_non_2xx_non_401_maps_to_comment_mutation_err() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_edit_comment(
        "PROJ-1".to_owned(),
        "10001".to_owned(),
        "updated".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(
        matches!(reply, Msg::CommentMutationErr(_)),
        "a non-2xx, non-401 update_comment response must still map to CommentMutationErr, never panic"
    );
}

// ---- c4b-delete-confirm-modal / ADR 0026 §4 / BDR 0017 S7 — delete_comment's
// reply mapping mirrors submit_comment's/edit_comment's: 2xx -> Ok,
// Unauthorized (401) -> the typed ClientError the spawn wrapper turns into
// the E2 re-auth guidance ----

#[tokio::test]
async fn delete_comment_2xx_is_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
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

    let result = delete_comment(&instance, "PROJ-1", "10001").await;

    assert!(result.is_ok(), "a 2xx delete_comment response must be Ok");
    server.verify().await;
}

#[tokio::test]
async fn delete_comment_401_maps_to_typed_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = delete_comment(&instance, "PROJ-1", "10001").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_delete_comment_2xx_replies_comment_mutation_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_delete_comment("PROJ-1".to_owned(), "10001".to_owned(), instance, tx);

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(matches!(reply, Msg::CommentMutationOk));
}

#[tokio::test]
async fn spawn_delete_comment_401_replies_comment_mutation_err_with_reauth_guidance() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_delete_comment("PROJ-1".to_owned(), "10001".to_owned(), instance, tx);

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    match reply {
        Msg::CommentMutationErr(reason) => {
            assert_eq!(reason, reauth_message("test"));
        }
        _ => panic!("expected Msg::CommentMutationErr, got a different Msg"),
    }
}

#[tokio::test]
async fn delete_comment_non_2xx_non_401_maps_to_comment_mutation_err() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_delete_comment("PROJ-1".to_owned(), "10001".to_owned(), instance, tx);

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(
        matches!(reply, Msg::CommentMutationErr(_)),
        "a non-2xx, non-401 delete_comment response must still map to CommentMutationErr, never panic"
    );
}

// ---- c4c-reply-mention / ADR 0026 §5 / BDR 0017 S8 — reply_comment's ADF
// carries a leading mention node + the body; reply mapping mirrors
// submit_comment's/edit_comment's/delete_comment's: 2xx -> Ok, Unauthorized
// (401) -> the typed ClientError the spawn wrapper turns into the E2
// re-auth guidance ----

fn expected_reply_adf_body(account_id: &str, display: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "body": {
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [
                    {"type": "mention", "attrs": {"id": account_id, "text": format!("@{display}")}},
                    {"type": "text", "text": " "},
                    {"type": "text", "text": text},
                ]
            }]
        }
    })
}

#[tokio::test]
async fn reply_comment_posts_v3_path_with_mention_adf_body_and_is_ok() {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .and(body_json(expected_reply_adf_body(
            "acct-B", "Bob", "Thanks!",
        )))
        .respond_with(ResponseTemplate::new(201).set_body_json(build_comment_payload_with_id("1")))
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

    let result = reply_comment(&instance, "PROJ-1", "acct-B", "Bob", "Thanks!").await;

    assert!(result.is_ok(), "a 2xx reply_comment response must be Ok");
    server.verify().await;
}

#[tokio::test]
async fn reply_comment_401_maps_to_typed_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };

    let result = reply_comment(&instance, "PROJ-1", "acct-B", "Bob", "Thanks!").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_reply_comment_2xx_replies_comment_mutation_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(build_comment_payload_with_id("1")))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_reply_comment(
        "PROJ-1".to_owned(),
        "acct-B".to_owned(),
        "Bob".to_owned(),
        "Thanks!".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(matches!(reply, Msg::CommentMutationOk));
}

#[tokio::test]
async fn spawn_reply_comment_401_replies_comment_mutation_err_with_reauth_guidance() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_reply_comment(
        "PROJ-1".to_owned(),
        "acct-B".to_owned(),
        "Bob".to_owned(),
        "Thanks!".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    match reply {
        Msg::CommentMutationErr(reason) => {
            assert_eq!(reason, reauth_message("test"));
        }
        _ => panic!("expected Msg::CommentMutationErr, got a different Msg"),
    }
}

#[tokio::test]
async fn reply_comment_non_2xx_non_401_maps_to_comment_mutation_err() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let instance = crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: server.uri(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    spawn_reply_comment(
        "PROJ-1".to_owned(),
        "acct-B".to_owned(),
        "Bob".to_owned(),
        "Thanks!".to_owned(),
        instance,
        tx,
    );

    let reply = rx
        .recv()
        .await
        .expect("the spawn must send exactly one reply");
    assert!(
        matches!(reply, Msg::CommentMutationErr(_)),
        "a non-2xx, non-401 reply_comment response must still map to CommentMutationErr, never panic"
    );
}
