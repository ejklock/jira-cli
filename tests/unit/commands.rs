use super::*;
use crate::config::Config;
use crate::i18n::{set_language, LANG_MUTEX};
use crate::models::{Issue, IssueAssignee, IssueComment};
use crate::store::cache::{IssueCache, TaskCache};
use crate::store::instances::{Instance, InstanceRepository};
use crate::store::Store;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn server_instance(server: &MockServer, name: &str) -> Instance {
    Instance {
        name: name.to_owned(),
        base_url: server.uri(),
        email: format!("{name}@example.com"),
        token: "test-token".to_owned(),
        account_id: Some("acc-42".to_owned()),
    }
}

fn make_store() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = Config {
        db_path,
        task_cache_ttl_hours: 24,
    };
    let store = Store::open(&config).unwrap();
    (dir, store)
}

fn sample_instance(store: &Store, name: &str) -> Instance {
    let inst = Instance {
        name: name.to_owned(),
        base_url: format!("https://{name}.atlassian.net"),
        email: format!("{name}@example.com"),
        token: format!("tok-{name}"),
        account_id: Some("acc-42".to_string()),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();
    inst
}

fn output_str(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).unwrap()
}

fn build_myself_payload() -> serde_json::Value {
    serde_json::json!({
        "accountId": "5b10a2844c20165700ede21g",
        "displayName": "Alice Example",
        "emailAddress": "alice@example.com",
        "active": true,
        "self": "https://example.atlassian.net/rest/api/3/user?accountId=5b10a2844c20165700ede21g",
        "avatarUrls": {}
    })
}

// ---- pick_instance tests ----

#[test]
fn pick_instance_empty_list_returns_err2() {
    let mut err = Vec::new();
    let result = pick_instance(&[], None, &mut err);
    assert_eq!(result, Err(2));
}

#[test]
fn pick_instance_single_no_name_returns_0() {
    let (_dir, store) = make_store();
    let inst = sample_instance(&store, "work");
    let mut err = Vec::new();
    let result = pick_instance(&[inst], None, &mut err);
    assert_eq!(result, Ok(0));
}

#[test]
fn pick_instance_by_name_returns_correct_index() {
    let (_dir, store) = make_store();
    let inst_a = sample_instance(&store, "alpha");
    let inst_b = sample_instance(&store, "beta");
    let instances = vec![inst_a, inst_b];
    let mut err = Vec::new();
    let result = pick_instance(&instances, Some("beta"), &mut err);
    assert_eq!(result, Ok(1));
}

#[test]
fn pick_instance_unknown_name_returns_err2() {
    let (_dir, store) = make_store();
    let inst = sample_instance(&store, "work");
    let mut err = Vec::new();
    let result = pick_instance(&[inst], Some("nope"), &mut err);
    assert_eq!(result, Err(2));
    assert!(output_str(&err).contains("not found"));
}

#[test]
fn pick_instance_multiple_no_name_returns_err2() {
    let (_dir, store) = make_store();
    let inst_a = sample_instance(&store, "alpha");
    let inst_b = sample_instance(&store, "beta");
    let instances = vec![inst_a, inst_b];
    let mut err = Vec::new();
    let result = pick_instance(&instances, None, &mut err);
    assert_eq!(result, Err(2));
}

// ---- setup_list tests ----

#[test]
fn setup_list_empty_prints_no_instances() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let code = setup_list(&repo, &mut out);
    assert_eq!(code, 0);
    assert!(output_str(&out).contains("No instances"));
}

#[test]
fn setup_list_with_instances_prints_header_and_rows() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    sample_instance(&store, "work");
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let code = setup_list(&repo, &mut out);
    assert_eq!(code, 0);
    let text = output_str(&out);
    assert!(text.contains("NAME"));
    assert!(text.contains("ACCOUNT_ID"));
    assert!(text.contains("work"));
}

// ---- setup_remove tests ----

#[test]
fn setup_remove_missing_instance_returns_2() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_remove(&repo, &cache, "ghost", &mut out, &mut err);
    assert_eq!(code, 2);
}

#[test]
fn setup_remove_existing_instance_returns_0() {
    let (_dir, store) = make_store();
    sample_instance(&store, "work");
    let repo = InstanceRepository::new(store.conn());
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_remove(&repo, &cache, "work", &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(output_str(&out).contains("removed"));
}

// ---- setup_add integration tests (BDR 0002) ----

/// Scenario 1: add happy path — valid fields + good token => stores instance + prints saved + OK
#[tokio::test]
async fn setup_add_happy_path_stores_instance_and_prints_saved() {
    {
        let _lock = LANG_MUTEX.lock().unwrap();
        set_language("en");
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_add(
        SetupAddFields {
            name: Some("myinstance".to_string()),
            url: Some(server.uri()),
            email: Some("alice@example.com".to_string()),
        },
        Some("valid-api-token".to_string()),
        &repo,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));
    let text = output_str(&out);
    assert!(
        text.contains("Instance 'myinstance' saved."),
        "must print saved message; got: {text}"
    );
    assert!(
        text.contains("Connectivity: OK"),
        "must print Connectivity: OK; got: {text}"
    );

    // Verify the stored row carries account_id
    let all = repo.load_all().unwrap();
    assert_eq!(all.len(), 1, "exactly one instance stored");
    assert_eq!(all[0].name, "myinstance");
    assert_eq!(
        all[0].account_id.as_deref(),
        Some("5b10a2844c20165700ede21g"),
        "account_id must be resolved from /myself response"
    );

    server.verify().await;
}

/// Scenario 2: missing required field (no email) => required-fields error, exit 2, nothing stored
#[tokio::test]
async fn setup_add_missing_field_returns_exit2_and_stores_nothing() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_add(
        SetupAddFields {
            name: Some("myinstance".to_string()),
            url: Some("https://myorg.atlassian.net".to_string()),
            email: None,
        },
        Some("some-token".to_string()),
        &repo,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "missing field must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("required"),
        "must print required-fields error; got: {err_text}"
    );
    let all = repo.load_all().unwrap();
    assert!(
        all.is_empty(),
        "nothing must be stored on validation failure"
    );
}

/// Scenario 3: bad token => /myself returns 401 => FAILED (HTTP 401), exit 1, nothing stored
#[tokio::test]
async fn setup_add_bad_token_prints_connectivity_failed_and_exits_1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_add(
        SetupAddFields {
            name: Some("badauth".to_string()),
            url: Some(server.uri()),
            email: Some("alice@example.com".to_string()),
        },
        Some("wrong-token".to_string()),
        &repo,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "bad token must exit 1");
    let out_text = output_str(&out);
    assert!(
        out_text.contains("Connectivity: FAILED"),
        "must print Connectivity: FAILED; got: {out_text}"
    );
    assert!(
        out_text.contains("401"),
        "must include HTTP 401 in FAILED message; got: {out_text}"
    );

    let all = repo.load_all().unwrap();
    assert!(all.is_empty(), "nothing must be stored on auth failure");

    server.verify().await;
}

/// Scenario 4: list empty => empty notice, exit 0 (also verified in setup_list_empty_prints_no_instances)
#[test]
fn setup_add_scenario4_list_empty_exit0() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let code = setup_list(&repo, &mut out);
    assert_eq!(code, 0);
    let text = output_str(&out);
    assert!(
        text.contains("No instances configured"),
        "must show empty notice; got: {text}"
    );
}

/// Scenario 5: remove missing => not-found, exit 2
#[test]
fn setup_remove_missing_prints_not_found_exit2() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_remove(&repo, &cache, "nope", &mut out, &mut err);
    assert_eq!(code, 2, "remove missing must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("not found"),
        "must print not-found error; got: {err_text}"
    );
}

/// Scenario 6: token is never echoed to stdout during setup_add
#[tokio::test]
async fn setup_add_token_never_appears_in_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let secret_token = "super-secret-api-token-12345";

    setup_add(
        SetupAddFields {
            name: Some("sectest".to_string()),
            url: Some(server.uri()),
            email: Some("alice@example.com".to_string()),
        },
        Some(secret_token.to_string()),
        &repo,
        &mut out,
        &mut err,
    )
    .await;

    let out_text = output_str(&out);
    assert!(
        !out_text.contains(secret_token),
        "API token must never appear in stdout; got: {out_text}"
    );
    let err_text = output_str(&err);
    assert!(
        !err_text.contains(secret_token),
        "API token must never appear in stderr; got: {err_text}"
    );
}

// ---- parse_issue_ref unit tests (BDR 0001) ----

#[test]
fn parse_issue_ref_bare_key_returns_key() {
    let result = parse_issue_ref("PROJ-123");
    assert_eq!(
        result.as_deref(),
        Some("PROJ-123"),
        "bare key must return key"
    );
}

#[test]
fn parse_issue_ref_browse_url_returns_key() {
    let result = parse_issue_ref("https://acme.atlassian.net/browse/PROJ-123");
    assert_eq!(
        result.as_deref(),
        Some("PROJ-123"),
        "browse URL must return key"
    );
}

#[test]
fn parse_issue_ref_browse_url_with_query_returns_key() {
    let result = parse_issue_ref("https://acme.atlassian.net/browse/PROJ-456?param=1");
    assert_eq!(
        result.as_deref(),
        Some("PROJ-456"),
        "URL with query must return key"
    );
}

#[test]
fn parse_issue_ref_invalid_returns_none() {
    assert_eq!(
        parse_issue_ref("not-a-key"),
        None,
        "plain text must return None"
    );
    assert_eq!(parse_issue_ref("123"), None, "bare number must return None");
    assert_eq!(parse_issue_ref(""), None, "empty string must return None");
    assert_eq!(
        parse_issue_ref("https://example.com/notjira"),
        None,
        "non-jira URL must return None"
    );
}

#[test]
fn parse_issue_ref_lowercase_key_returns_none() {
    assert_eq!(
        parse_issue_ref("proj-123"),
        None,
        "lowercase key must not match"
    );
}

// ---- get_core integration tests (BDR 0001) ----

fn build_issue_payload() -> serde_json::Value {
    serde_json::json!({
        "id": "10001",
        "key": "PROJ-123",
        "self": "https://example.atlassian.net/rest/api/3/issue/10001",
        "fields": {
            "summary": "Fix the login bug",
            "status": {
                "id": "3",
                "name": "In Progress",
                "description": "Being worked on",
                "iconUrl": "https://example.atlassian.net/images/icons/statuses/inprogress.png",
                "self": "https://example.atlassian.net/rest/api/3/status/3",
                "statusCategory": {
                    "id": 4,
                    "key": "indeterminate",
                    "colorName": "yellow",
                    "name": "In Progress"
                }
            },
            "issuetype": {
                "id": "10002",
                "name": "Bug",
                "description": "A problem",
                "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/bug.png",
                "self": "https://example.atlassian.net/rest/api/3/issuetype/10002",
                "subtask": false
            },
            "assignee": {
                "accountId": "acc-alice",
                "displayName": "Alice",
                "active": true,
                "self": "https://example.atlassian.net/rest/api/3/user?accountId=acc-alice",
                "avatarUrls": {}
            },
            "reporter": {
                "accountId": "acc-reporter",
                "displayName": "Reporter Name",
                "active": true,
                "self": "https://example.atlassian.net/rest/api/3/user?accountId=acc-reporter",
                "avatarUrls": {}
            },
            "priority": {
                "id": "2",
                "name": "High",
                "iconUrl": "https://example.atlassian.net/images/icons/priorities/high.png",
                "self": "https://example.atlassian.net/rest/api/3/priority/2"
            },
            "created": "2026-01-10T09:00:00.000+0000",
            "updated": "2026-06-29T12:00:00.000+0000",
            "description": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Login fails when MFA is enabled." }]
                }]
            },
            "comment": {
                "comments": [{
                    "id": "100",
                    "self": "https://example.atlassian.net/rest/api/3/issue/10001/comment/100",
                    "author": {
                        "accountId": "acc-bob",
                        "displayName": "Bob Dev",
                        "active": true,
                        "self": "https://example.atlassian.net/rest/api/3/user?accountId=acc-bob",
                        "avatarUrls": {}
                    },
                    "body": "Reproduced on v2.1.",
                    "created": "2026-06-29T10:00:00.000+0000",
                    "updated": "2026-06-29T10:00:00.000+0000"
                }],
                "maxResults": 1,
                "total": 1,
                "startAt": 0
            }
        }
    })
}

/// Scenario 1 (BDR 0001): get by bare key — fetches, writes cache, renders human output, exit 0.
#[tokio::test]
async fn get_core_by_bare_key_fetches_and_renders_human_output() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));
    let text = output_str(&out);
    assert!(
        text.contains("Fix the login bug"),
        "must render summary: {text}"
    );
    assert!(text.contains("PROJ-123"), "must render key: {text}");
    assert!(text.contains("In Progress"), "must render status: {text}");
    assert!(text.contains("Alice"), "must render assignee: {text}");
    assert!(
        text.contains("Login fails when MFA is enabled."),
        "must render ADF-flattened description: {text}"
    );

    // Cache must be written (ADR 0003).
    let issue_cache = IssueCache::new(store.conn());
    let cached = issue_cache.read("work", "PROJ-123").unwrap();
    assert!(cached.is_some(), "issue must be cached after get");
    assert_eq!(cached.unwrap().issue.key, "PROJ-123");

    server.verify().await;
}

/// Scenario 2 (BDR 0001): get by browse URL — resolves key and behaves identically.
#[tokio::test]
async fn get_core_by_browse_url_resolves_key_and_fetches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: None,
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let url = format!("{}/browse/PROJ-123", server.uri());
    let code = get_core(
        &url,
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(
        code,
        0,
        "browse URL must exit 0; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out);
    assert!(text.contains("PROJ-123"), "must render issue key: {text}");

    server.verify().await;
}

/// Scenario 3 (BDR 0001): --json outputs a single minified line with ref==key.
#[tokio::test]
async fn get_core_json_flag_outputs_minified_object_with_ref() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: None,
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: true,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0; stderr: {}", output_str(&err));
    let text = output_str(&out).trim().to_string();
    // Must be exactly one non-empty line.
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json output must be exactly 1 line: {text:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(obj["ref"], "PROJ-123", "ref must equal the issue key");
    assert_eq!(
        obj["status_category"], "indeterminate",
        "status_category must be the key"
    );
    assert!(obj.get("summary").is_some(), "must have summary field");
    assert!(
        obj.get("issue_type").is_some(),
        "must have issue_type field"
    );

    server.verify().await;
}

/// Scenario 4 (BDR 0001): 404 → not-found error, exit 1.
#[tokio::test]
async fn get_core_not_found_404_returns_exit1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-404"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Issue not found"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: None,
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-404",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "not found must exit 1");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("not found") || err_text.contains("404"),
        "must print not-found error; stderr: {err_text}"
    );

    server.verify().await;
}

/// Scenario 5 (BDR 0001): ambiguous instance → exit 2, zero network requests.
#[tokio::test]
async fn get_core_ambiguous_instance_exits_2_no_network() {
    let server = MockServer::start().await;
    // No mocks mounted — any request would panic with "unexpected request".

    let (_dir, store) = make_store();
    let inst_a = sample_instance(&store, "alpha");
    let inst_b = sample_instance(&store, "beta");
    let instances = vec![inst_a.clone(), inst_b];
    let mut err_buf = Vec::new();
    // pick_instance with two instances and no name => error 2
    let result = pick_instance(&instances, None, &mut err_buf);
    assert_eq!(result, Err(2), "ambiguous instance must return Err(2)");
    let err_text = output_str(&err_buf);
    assert!(
        err_text.contains("multiple instances") || err_text.contains("Use --instance"),
        "must print ambiguity message; got: {err_text}"
    );

    // Confirm the server received no requests.
    server.verify().await;
    let _ = inst_a;
}

/// Scenario 6 (BDR 0001): --no-comments → comments:[] in JSON, omits human block.
#[tokio::test]
async fn get_core_no_comments_flag_suppresses_comments() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: None,
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: true,
            no_comments: true,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0; stderr: {}", output_str(&err));
    let text = output_str(&out).trim().to_string();
    let obj: serde_json::Value = serde_json::from_str(&text).expect("must be valid JSON");
    assert_eq!(
        obj["comments"].as_array().unwrap().len(),
        0,
        "no-comments must yield empty comments array in JSON"
    );

    server.verify().await;
}

/// Bad ref → exit 2 without any network request.
#[test]
fn get_core_invalid_ref_parses_to_none() {
    let result = parse_issue_ref("not-valid");
    assert!(result.is_none(), "bad ref must not parse");
}

// ---- get_core local-first cache tests (BDR 0003) ----

fn build_updated_issue_payload() -> serde_json::Value {
    let mut payload = build_issue_payload();
    payload["fields"]["summary"] = serde_json::json!("Updated summary after refresh");
    payload
}

fn pre_populate_cache(store: &Store, instance_name: &str) {
    let issue = Issue {
        key: "PROJ-123".to_string(),
        summary: "Cached summary before refresh".to_string(),
        status: "Open".to_string(),
        status_category: Some("new".to_string()),
        issue_type: "Bug".to_string(),
        assignee: Some(IssueAssignee {
            display_name: "Alice".to_string(),
            account_id: Some("acc-alice".to_string()),
        }),
        reporter: None,
        priority: None,
        created: None,
        updated: None,
        duedate: None,
        description: None,
        comments: vec![IssueComment {
            id: Some("1".to_string()),
            author: None,
            author_account_id: None,
            body: "Cached comment".to_string(),
            created: None,
            updated: None,
        }],
        attachments: vec![],
    };
    IssueCache::new(store.conn())
        .write(instance_name, &issue)
        .unwrap();
}

/// BDR 0003 Scenario 1: offline cache hit — 0 HTTP requests, exit 0, renders from cache.
#[tokio::test]
async fn get_core_offline_cache_hit_makes_zero_requests() {
    let server = MockServer::start().await;

    let (_dir, store) = make_store();
    pre_populate_cache(&store, "work");

    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 on cache hit; stderr: {}", output_str(&err));
    let text = output_str(&out);
    assert!(
        text.contains("Cached summary before refresh"),
        "must render cached summary; got: {text}"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "cache hit must make zero HTTP requests; received {requests:?}"
    );
}

/// BDR 0003 Scenario 2: --refresh re-fetches exactly once and overwrites the cached row.
#[tokio::test]
async fn get_core_refresh_fetches_once_and_overwrites_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_updated_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    pre_populate_cache(&store, "work");

    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: true,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(
        code,
        0,
        "exit 0 after successful refresh; stderr: {}",
        output_str(&err)
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        1,
        "--refresh must issue exactly 1 HTTP request; got {requests:?}"
    );

    let cached = IssueCache::new(store.conn())
        .read("work", "PROJ-123")
        .unwrap()
        .expect("cache row must exist after refresh");
    assert_eq!(
        cached.issue.summary, "Updated summary after refresh",
        "cache row must hold the refreshed summary"
    );

    server.verify().await;
}

/// BDR 0003 Scenario 3: --refresh with unreachable network exits 1 and leaves prior cache row intact.
#[tokio::test]
async fn get_core_refresh_network_failure_leaves_cache_intact() {
    let (_dir, store) = make_store();
    pre_populate_cache(&store, "work");

    let inst = Instance {
        name: "work".to_string(),
        base_url: "http://127.0.0.1:19999".to_string(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: true,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "refresh with unreachable host must exit 1");

    let cached = IssueCache::new(store.conn())
        .read("work", "PROJ-123")
        .unwrap()
        .expect("prior cache row must still exist after failed refresh");
    assert_eq!(
        cached.issue.summary, "Cached summary before refresh",
        "prior cache row must be unchanged after failed refresh"
    );

    let subsequent_inst = unreachable_instance("work");
    let subsequent_code = get_core(
        "PROJ-123",
        &subsequent_inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .await;
    assert_eq!(
        subsequent_code, 0,
        "subsequent offline read must still succeed after failed refresh"
    );
}

fn unreachable_instance(name: &str) -> Instance {
    Instance {
        name: name.to_string(),
        base_url: "http://127.0.0.1:19999".to_string(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    }
}

// ---- current_core integration tests (BDR 0004) ----

/// BDR 0004 Scenario 1: prefixed branch with a valid key → resolves key, fetches issue, renders, exit 0.
#[tokio::test]
async fn current_core_prefixed_branch_renders_issue_and_exits_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        Some("feature/PROJ-123-add-login"),
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(
        code,
        0,
        "exit 0 for valid branch; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out);
    assert!(
        text.contains("Fix the login bug"),
        "must render summary: {text}"
    );
    assert!(text.contains("PROJ-123"), "must render issue key: {text}");

    server.verify().await;
}

/// BDR 0004 Scenario 4: --json passthrough → minified JSON output for the branch's issue.
#[tokio::test]
async fn current_core_json_flag_passes_through_to_get_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        Some("feature/PROJ-123-add-login"),
        &inst,
        &cache,
        GetOpts {
            json: true,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 with --json; stderr: {}", output_str(&err));
    let text = output_str(&out).trim().to_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json output must be exactly one line: {text:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(obj["ref"], "PROJ-123", "ref must equal the issue key");
    assert!(obj.get("summary").is_some(), "must have summary field");

    server.verify().await;
}

/// BDR 0004 Scenario 3: branch with no issue key → error message, exit 2, zero requests.
#[tokio::test]
async fn current_core_branch_without_key_exits_2_with_zero_requests() {
    let server = MockServer::start().await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        Some("main"),
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no-key branch must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("no issue key in branch"),
        "must describe the missing key; stderr: {err_text}"
    );
    assert!(
        err_text.contains("main"),
        "must include the branch name; stderr: {err_text}"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "no-key guard must make zero HTTP requests"
    );
}

/// BDR 0004 Scenario 5 (no-repo): branch is None → error message, exit 2, zero requests.
#[tokio::test]
async fn current_core_not_in_git_repo_exits_2_with_zero_requests() {
    let server = MockServer::start().await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        None,
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "not-a-repo must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("not in a git repository") || err_text.contains("no current branch"),
        "must explain the missing branch; stderr: {err_text}"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "not-a-repo guard must make zero HTTP requests"
    );
}

/// BDR 0016 S9: `current_core` in agent mode is byte-identical to `get_core`
/// called directly with the key `current_core` extracts from the branch —
/// the invariant `dispatch_current`'s interactive routing relies on to seed
/// `browse_seeded(TuiSeed::Detail(key))` with the SAME key agent mode would
/// have fetched, so the two modes never diverge on which issue they open.
#[tokio::test]
async fn current_core_agent_mode_output_matches_get_core_with_extracted_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());

    let mut current_out = Vec::new();
    let mut current_err = Vec::new();
    let current_code = current_core(
        Some("feature/PROJ-123-add-login"),
        &inst,
        &cache,
        GetOpts {
            json: true,
            no_comments: false,
            refresh: false,
        },
        &mut current_out,
        &mut current_err,
    )
    .await;

    let mut get_out = Vec::new();
    let mut get_err = Vec::new();
    let get_code = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: true,
            no_comments: false,
            refresh: false,
        },
        &mut get_out,
        &mut get_err,
    )
    .await;

    assert_eq!(
        current_code, get_code,
        "current_core and get_core(extracted key) must exit identically"
    );
    assert_eq!(
        output_str(&current_out),
        output_str(&get_out),
        "current_core output must be byte-identical to get_core with the branch-extracted key"
    );
    assert!(
        output_str(&current_err).is_empty() && output_str(&get_err).is_empty(),
        "neither call should write to stderr on success"
    );

    server.verify().await;
}

/// BDR 0003 Scenario 4: miss then hit — first get makes 1 request and writes cache;
/// second offline get makes 0 requests.
#[tokio::test]
async fn get_core_miss_then_hit_lifecycle() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_string(),
        base_url: server.uri(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let cache = TaskCache::new(store.conn());

    let code_first = get_core(
        "PROJ-123",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .await;
    assert_eq!(code_first, 0, "first get (miss) must exit 0");

    let requests_after_first = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests_after_first.len(),
        1,
        "first get must make exactly 1 HTTP request"
    );

    let offline_inst = Instance {
        name: "work".to_string(),
        base_url: "http://127.0.0.1:19999".to_string(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        account_id: Some("acc-42".to_string()),
    };
    let mut out = Vec::new();
    let code_second = get_core(
        "PROJ-123",
        &offline_inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut Vec::new(),
    )
    .await;
    assert_eq!(code_second, 0, "second get (hit) must exit 0");

    let text = output_str(&out);
    assert!(
        text.contains("Fix the login bug"),
        "second get must render from cache; got: {text}"
    );

    let requests_after_second = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests_after_second.len(),
        1,
        "second get must make 0 new HTTP requests (still exactly 1 total)"
    );

    server.verify().await;
}

// ---- mine_core integration tests (BDR 0005) ----

use super::MINE_JQL;

fn build_search_payload(issues: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "issues": issues,
        "isLast": true,
        "nextPageToken": null
    })
}

fn build_search_issue(
    key: &str,
    issue_type: &str,
    status: &str,
    assignee_name: Option<&str>,
    summary: &str,
) -> serde_json::Value {
    let assignee = match assignee_name {
        Some(name) => serde_json::json!({
            "accountId": "u1",
            "displayName": name,
            "active": true,
            "self": "",
            "avatarUrls": {}
        }),
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "id": "10001",
        "key": key,
        "self": "",
        "fields": {
            "summary": summary,
            "status": {
                "id": "1",
                "name": status,
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
                "name": issue_type,
                "description": "",
                "iconUrl": "",
                "self": "",
                "subtask": false
            },
            "assignee": assignee,
            "priority": {
                "id": "3",
                "name": "Medium",
                "iconUrl": "",
                "self": ""
            },
            "created": "2026-01-01T00:00:00.000+0000",
            "updated": "2026-06-29T00:00:00.000+0000"
        }
    })
}

fn save_instance_for_server(store: &Store, server: &MockServer, name: &str) -> Instance {
    let inst = Instance {
        name: name.to_owned(),
        base_url: server.uri(),
        email: format!("{name}@example.com"),
        token: format!("tok-{name}"),
        account_id: Some("acc-42".to_string()),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();
    inst
}

/// BDR 0005 Scenario 1 (AC1): mine_core builds EXACTLY the mine JQL, renders table with
/// KEY/TYPE/STATUS/ASSIGNEE/SUMMARY columns, and exits 0.
/// The wiremock-captured request must carry the exact JQL in its query string.
#[tokio::test]
async fn mine_core_builds_exact_jql_renders_table_and_exits_0() {
    let server = MockServer::start().await;

    let issues = serde_json::json!([
        build_search_issue("PROJ-1", "Bug", "Open", Some("Alice"), "Fix the crash"),
        build_search_issue("PROJ-2", "Task", "In Progress", None, "Refactor module"),
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .and(query_param("jql", MINE_JQL))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, None, &mut out, &mut err).await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));

    let text = output_str(&out);
    assert!(text.contains("KEY"), "table must have KEY column: {text}");
    assert!(text.contains("TYPE"), "table must have TYPE column: {text}");
    assert!(
        text.contains("STATUS"),
        "table must have STATUS column: {text}"
    );
    assert!(
        text.contains("ASSIGNEE"),
        "table must have ASSIGNEE column: {text}"
    );
    assert!(
        text.contains("SUMMARY"),
        "table must have SUMMARY column: {text}"
    );
    assert!(
        text.contains("PROJ-1"),
        "must contain first issue key: {text}"
    );
    assert!(text.contains("Bug"), "must contain issue type: {text}");
    assert!(text.contains("Open"), "must contain status: {text}");
    assert!(text.contains("Alice"), "must contain assignee: {text}");
    assert!(
        text.contains("Fix the crash"),
        "must contain summary: {text}"
    );
    assert!(
        text.contains("PROJ-2"),
        "must contain second issue key: {text}"
    );
    assert!(
        text.contains("Unassigned"),
        "None assignee must render as Unassigned: {text}"
    );

    // AC1: the request the server received must carry the exact mine JQL.
    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1, "must issue exactly 1 search request");
    let req_url = requests[0].url.as_str();
    let decoded_url = urlencoding_decode(req_url);
    assert!(
        decoded_url.contains(MINE_JQL),
        "request URL must carry the exact mine JQL; got: {decoded_url}"
    );

    server.verify().await;
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            let hex = format!("{h1}{h2}");
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

/// BDR 0005 Scenario 5 (AC2): empty result -> stdout "No issues.", exit 0.
#[tokio::test]
async fn mine_core_empty_result_prints_no_issues_and_exits_0() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload(serde_json::json!([]))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, None, &mut out, &mut err).await;

    assert_eq!(
        code,
        0,
        "empty result must exit 0; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out);
    assert_eq!(
        text.trim(),
        "No issues.",
        "empty result must print exactly 'No issues.'; got: {text:?}"
    );

    server.verify().await;
}

/// AC3 / NFR-1: Token host isolation — with two configured instances, mine filtered to
/// instance A contacts only A's host; B's server receives zero requests.
#[tokio::test]
async fn mine_core_host_isolation_only_contacts_selected_instance() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let issues = serde_json::json!([build_search_issue(
        "PROJ-1",
        "Task",
        "Open",
        Some("Alice"),
        "Work item"
    ),]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server_a)
        .await;

    // Server B must receive zero requests.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server_b)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server_a, "instance-a");
    save_instance_for_server(&store, &server_b, "instance-b");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, Some("instance-a"), false, None, &mut out, &mut err).await;

    assert_eq!(
        code,
        0,
        "filtered mine must exit 0; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out);
    assert!(
        text.contains("PROJ-1"),
        "must render instance-a's issue: {text}"
    );

    let b_requests = server_b.received_requests().await.unwrap_or_default();
    assert_eq!(
        b_requests.len(),
        0,
        "instance-b must receive zero requests; got: {b_requests:?}"
    );

    server_a.verify().await;
    server_b.verify().await;
}

/// AC1 edge: mine_core with no configured instances exits 2 (pick_instance guard), zero requests.
#[tokio::test]
async fn mine_core_no_instances_exits_2() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, None, &mut out, &mut err).await;

    assert_eq!(code, 2, "no instances must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("no instances configured") || err_text.contains("No instances"),
        "must print no-instances error; got: {err_text}"
    );
}

/// BDR 0005 Scenario 2 (AC1): mine_core with json=true emits a single-line {count, jql, issues}
/// object where jql == MINE_JQL; exit 0.
#[tokio::test]
async fn mine_core_json_flag_emits_single_line_list_object_exit_0() {
    let server = MockServer::start().await;

    let issues = serde_json::json!([
        build_search_issue("PROJ-1", "Bug", "Open", Some("Alice"), "Fix the crash"),
        build_search_issue("PROJ-2", "Task", "In Progress", None, "Refactor module"),
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, true, None, &mut out, &mut err).await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));
    let text = output_str(&out).trim().to_owned();

    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json output must be exactly 1 line: {text:?}"
    );

    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert!(obj.get("count").is_some(), "must have count field");
    assert!(obj.get("jql").is_some(), "must have jql field");
    assert!(obj.get("issues").is_some(), "must have issues field");
    assert_eq!(
        obj["jql"], MINE_JQL,
        "jql field must equal the production MINE_JQL const"
    );
    assert_eq!(
        obj["count"].as_u64().unwrap(),
        2,
        "count must equal number of returned issues"
    );

    let issues_arr = obj["issues"].as_array().expect("issues must be an array");
    assert_eq!(issues_arr.len(), 2, "issues array must have 2 entries");

    server.verify().await;
}

/// BDR 0005 Scenario 5 (AC1): mine_core with json=true and 0 results emits
/// {"count":0,...} single line; exit 0.
#[tokio::test]
async fn mine_core_json_empty_result_emits_count_0_single_line_exit_0() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload(serde_json::json!([]))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, true, None, &mut out, &mut err).await;

    assert_eq!(
        code,
        0,
        "json empty must exit 0; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out).trim().to_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json empty output must be exactly 1 line: {text:?}"
    );

    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(
        obj["count"].as_u64().unwrap(),
        0,
        "count must be 0 for empty result"
    );
    let issues_arr = obj["issues"].as_array().expect("issues must be an array");
    assert_eq!(
        issues_arr.len(),
        0,
        "issues array must be empty for empty result"
    );

    server.verify().await;
}

/// BDR 0005 Scenario 6 (AC2): mine_core with limit=Some(5) sends maxResults=5 to the server.
#[tokio::test]
async fn mine_core_limit_sends_max_results_in_request() {
    let server = MockServer::start().await;

    let issues = serde_json::json!([build_search_issue(
        "PROJ-1",
        "Task",
        "Open",
        Some("Alice"),
        "Item one"
    ),]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .and(query_param("maxResults", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, Some(5), &mut out, &mut err).await;

    assert_eq!(
        code,
        0,
        "--limit 5 must exit 0; stderr: {}",
        output_str(&err)
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1, "must issue exactly 1 search request");
    let req_url = requests[0].url.as_str();
    assert!(
        req_url.contains("maxResults=5"),
        "request URL must carry maxResults=5; got: {req_url}"
    );

    server.verify().await;
}

// ---- search_core integration tests (BDR 0005 / J4) ----

/// AC1 (BDR 0005 Scn 3 — verbatim): search_core sends the user JQL verbatim to
/// GET /rest/api/3/search/jql, renders a human table with expected columns, and exits 0.
/// The wiremock-captured request URL must carry the exact user JQL unchanged.
#[tokio::test]
async fn search_core_sends_jql_verbatim_renders_table_exits_0() {
    let server = MockServer::start().await;
    let user_jql = "project = PROJ ORDER BY updated DESC";

    let issues = serde_json::json!([
        build_search_issue("PROJ-10", "Bug", "Open", Some("Bob"), "Critical bug"),
        build_search_issue("PROJ-11", "Task", "In Progress", None, "Cleanup task"),
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .and(query_param("jql", user_jql))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let instance = save_instance_for_server(&store, &server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some(user_jql), &instance, false, &mut out, &mut err).await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));

    let text = output_str(&out);
    assert!(text.contains("KEY"), "table must have KEY column: {text}");
    assert!(text.contains("TYPE"), "table must have TYPE column: {text}");
    assert!(
        text.contains("STATUS"),
        "table must have STATUS column: {text}"
    );
    assert!(
        text.contains("ASSIGNEE"),
        "table must have ASSIGNEE column: {text}"
    );
    assert!(
        text.contains("SUMMARY"),
        "table must have SUMMARY column: {text}"
    );
    assert!(
        text.contains("PROJ-10"),
        "must contain first issue key: {text}"
    );
    assert!(text.contains("Bob"), "must contain assignee: {text}");
    assert!(
        text.contains("Critical bug"),
        "must contain first summary: {text}"
    );
    assert!(
        text.contains("PROJ-11"),
        "must contain second issue key: {text}"
    );
    assert!(
        text.contains("Unassigned"),
        "None assignee must render as Unassigned: {text}"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1, "must issue exactly 1 search request");
    let decoded_url = urlencoding_decode(requests[0].url.as_str());
    assert!(
        decoded_url.contains(user_jql),
        "request URL must carry the exact user JQL verbatim; got: {decoded_url}"
    );

    server.verify().await;
}

/// AC2 (BDR 0005 Scn 3 — json): search_core with json=true emits a single-line
/// {count, jql, issues} object whose jql field equals the user input verbatim; exit 0.
#[tokio::test]
async fn search_core_json_flag_emits_list_object_with_verbatim_jql_exits_0() {
    let server = MockServer::start().await;
    let user_jql = "project = PROJ ORDER BY updated DESC";

    let issues = serde_json::json!([
        build_search_issue("PROJ-10", "Bug", "Open", Some("Bob"), "Critical bug"),
        build_search_issue("PROJ-11", "Task", "In Progress", None, "Cleanup task"),
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_search_payload(issues)))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let instance = save_instance_for_server(&store, &server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some(user_jql), &instance, true, &mut out, &mut err).await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));

    let text = output_str(&out).trim().to_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json output must be exactly 1 line: {text:?}"
    );

    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert!(obj.get("count").is_some(), "must have count field");
    assert!(obj.get("jql").is_some(), "must have jql field");
    assert!(obj.get("issues").is_some(), "must have issues field");
    assert_eq!(
        obj["jql"], user_jql,
        "jql field must equal the user JQL verbatim"
    );
    assert_eq!(
        obj["count"].as_u64().unwrap(),
        2,
        "count must equal number of returned issues"
    );

    let issues_arr = obj["issues"].as_array().expect("issues must be an array");
    assert_eq!(issues_arr.len(), 2, "issues array must have 2 entries");

    server.verify().await;
}

/// AC3 (BDR 0005 Scn 4 — invalid JQL): a Jira 400 response maps to stderr
/// containing 'invalid JQL' and exit 1.
#[tokio::test]
async fn search_core_invalid_jql_400_exits_1_with_invalid_jql_prefix() {
    let server = MockServer::start().await;
    let bad_jql = "project = ??? invalid syntax";

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["Error in the JQL Query: 'invalid syntax' is not a valid keyword."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let instance = save_instance_for_server(&store, &server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some(bad_jql), &instance, false, &mut out, &mut err).await;

    assert_eq!(code, 1, "invalid JQL must exit 1");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("invalid JQL"),
        "stderr must contain 'invalid JQL' prefix; got: {err_text}"
    );

    server.verify().await;
}

/// AC4 (BDR 0005 Scn 5 — empty): empty search result prints exactly 'No issues.' and exits 0.
#[tokio::test]
async fn search_core_empty_result_prints_no_issues_exits_0() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(build_search_payload(serde_json::json!([]))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let instance = save_instance_for_server(&store, &server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some("project = PROJ"), &instance, false, &mut out, &mut err).await;

    assert_eq!(
        code,
        0,
        "empty result must exit 0; stderr: {}",
        output_str(&err)
    );
    let text = output_str(&out);
    assert_eq!(
        text.trim(),
        "No issues.",
        "empty result must print exactly 'No issues.'; got: {text:?}"
    );

    server.verify().await;
}

/// Edge: search_core with None jql exits 2, zero network calls.
#[tokio::test]
async fn search_core_none_jql_exits_2_no_network() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let instance = server_instance(&server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(None, &instance, false, &mut out, &mut err).await;

    assert_eq!(code, 2, "None jql must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("search requires a JQL query"),
        "must explain that JQL is required; got: {err_text}"
    );
    assert!(
        output_str(&out).is_empty(),
        "stdout must be empty for validation error"
    );

    server.verify().await;
}

/// Edge: search_core with whitespace-only jql exits 2, zero network calls.
#[tokio::test]
async fn search_core_blank_jql_exits_2_no_network() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let instance = server_instance(&server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some("   "), &instance, false, &mut out, &mut err).await;

    assert_eq!(code, 2, "blank jql must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("search requires a JQL query"),
        "must explain that JQL is required; got: {err_text}"
    );

    server.verify().await;
}

// ---- setup_language tests (J5b / AC3) ----

#[test]
fn setup_language_pt_br_hyphen_stores_canonical_pt_underscore_br_exits_0() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = crate::store::settings::SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_language(&repo, Some("pt-BR"), &mut out, &mut err);

    assert_eq!(code, 0, "pt-BR must exit 0; stderr: {}", output_str(&err));
    let stored = repo.get("language", None).unwrap();
    assert_eq!(
        stored.as_deref(),
        Some("pt_BR"),
        "must store canonical 'pt_BR', not 'pt-BR'"
    );
    let out_text = output_str(&out);
    assert!(
        out_text.contains("pt_BR"),
        "stdout must confirm canonical code 'pt_BR'; got: {out_text}"
    );
}

#[test]
fn setup_language_none_after_set_prints_current_language() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = crate::store::settings::SettingsRepository::new(store.conn());

    let code_set = setup_language(&repo, Some("pt-BR"), &mut Vec::new(), &mut Vec::new());
    assert_eq!(code_set, 0, "set step must exit 0");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code_get = setup_language(&repo, None, &mut out, &mut err);

    assert_eq!(
        code_get,
        0,
        "get step must exit 0; stderr: {}",
        output_str(&err)
    );
    let out_text = output_str(&out);
    assert!(
        out_text.contains("pt_BR"),
        "get output must include stored canonical code 'pt_BR'; got: {out_text}"
    );
}

#[test]
fn setup_language_unsupported_code_prints_error_exits_2_stores_nothing() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = crate::store::settings::SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_language(&repo, Some("zz"), &mut out, &mut err);

    assert_eq!(code, 2, "unsupported code must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("unsupported language"),
        "stderr must contain 'unsupported language'; got: {err_text}"
    );
    let stored = repo.get("language", None).unwrap();
    assert!(
        stored.is_none(),
        "nothing must be stored for unsupported code; got: {stored:?}"
    );
}

#[test]
fn setup_language_en_stores_en_exits_0() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let (_dir, store) = make_store();
    let repo = crate::store::settings::SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = setup_language(&repo, Some("en"), &mut out, &mut err);

    assert_eq!(code, 0, "en must exit 0; stderr: {}", output_str(&err));
    let stored = repo.get("language", None).unwrap();
    assert_eq!(
        stored.as_deref(),
        Some("en"),
        "must store 'en'; got: {stored:?}"
    );
    let out_text = output_str(&out);
    assert!(
        out_text.contains("en"),
        "stdout must confirm 'en'; got: {out_text}"
    );
}

#[test]
fn cli_parses_setup_language_pt_br_to_language_args() {
    use crate::cli::{Cli, SetupCmd};
    use clap::Parser;

    let cli = Cli::try_parse_from(["jira", "setup", "language", "pt-BR"])
        .expect("must parse setup language pt-BR");
    let Some(crate::cli::Command::Setup(opts)) = cli.command else {
        panic!("expected Setup command");
    };
    let SetupCmd::Language(args) = opts.subcommand else {
        panic!("expected Language subcommand");
    };
    assert_eq!(
        args.code.as_deref(),
        Some("pt-BR"),
        "code must be Some('pt-BR')"
    );
}

// ---- R-E2: 401 re-auth messaging (AC2/AC4) ----

const EXPECTED_REAUTH_EN: &str =
    "Authentication failed for work: your API token was rejected. Run `jira setup add` to re-authenticate.";
const EXPECTED_REAUTH_PT_BR: &str =
    "Falha de autenticação em work: seu API token foi rejeitado. Rode `jira setup add` para se autenticar novamente.";

/// AC2: get_core on a 401 prints the exact actionable re-auth message (naming
/// the instance and `jira setup add`) to stderr and exits non-zero.
///
/// The lock is held across the `.await`s (rather than only around
/// `set_language`, ADR pattern for language-independent async tests): this
/// test asserts on translated (`tf()`) output, so a concurrent test flipping
/// the process-global language mid-fetch would flake the assertion.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn get_core_401_prints_reauth_message_and_exits_nonzero() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-401"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-401",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_ne!(code, 0, "401 must exit non-zero");
    let err_text = output_str(&err);
    assert!(
        err_text.contains(EXPECTED_REAUTH_EN),
        "stderr must contain the exact re-auth guidance; got: {err_text}"
    );
    server.verify().await;
}

/// AC2: the same 401 guidance renders in pt_BR under LANG_MUTEX (held across
/// the `.await`s for the same reason as the sibling English test above).
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn get_core_401_renders_pt_br_reauth_message() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-401"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-401",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    set_language("en");
    assert_ne!(code, 0, "401 must exit non-zero");
    let err_text = output_str(&err);
    assert!(
        err_text.contains(EXPECTED_REAUTH_PT_BR),
        "pt_BR stderr must contain the translated re-auth guidance; got: {err_text}"
    );
    server.verify().await;
}

/// AC4 no-drift guard: a non-401 error on get_core keeps its pre-existing
/// rendering — the 404 path stays exactly `Error: issue '<key>' not found.`,
/// never the re-auth message.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn get_core_404_error_rendering_unchanged_by_reauth_mapping() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-404"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Issue not found"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = server_instance(&server, "work");
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "PROJ-404",
        &inst,
        &cache,
        GetOpts {
            json: false,
            no_comments: false,
            refresh: false,
        },
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "404 must exit 1");
    let err_text = output_str(&err);
    assert_eq!(
        err_text.trim(),
        "Error: issue 'PROJ-404' not found.",
        "404 rendering must be byte-identical to before; got: {err_text:?}"
    );
    assert!(
        !err_text.contains("API token"),
        "a 404 must never render the re-auth guidance; got: {err_text}"
    );
    server.verify().await;
}

/// AC2: mine_core on a 401 prints the exact actionable re-auth message to
/// stderr and exits non-zero.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn mine_core_401_prints_reauth_message_and_exits_nonzero() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, None, &mut out, &mut err).await;

    assert_ne!(code, 0, "401 must exit non-zero");
    let err_text = output_str(&err);
    assert!(
        err_text.contains(EXPECTED_REAUTH_EN),
        "stderr must contain the exact re-auth guidance; got: {err_text}"
    );
    server.verify().await;
}

/// AC4 no-drift guard: a non-401 error on mine_core keeps rendering
/// `Error fetching issues: <detail>`, never the re-auth message.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn mine_core_500_error_rendering_unchanged_by_reauth_mapping() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    save_instance_for_server(&store, &server, "work");
    let repo = InstanceRepository::new(store.conn());

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = mine_core(&repo, None, false, None, &mut out, &mut err).await;

    assert_eq!(code, 1, "500 must exit 1");
    let err_text = output_str(&err);
    assert!(
        err_text.starts_with("Error fetching issues:"),
        "500 rendering must be byte-identical to before; got: {err_text:?}"
    );
    assert!(
        !err_text.contains("API token"),
        "a 500 must never render the re-auth guidance; got: {err_text}"
    );
    server.verify().await;
}

/// AC3 (boundary): the TUI shell's `run_search` — the fetch every
/// `Msg::LoadFailed` spawn site in `src/tui/shell.rs` is built on — surfaces
/// the same typed `ClientError::Unauthorized` on a 401 that the CLI matches
/// on. The Msg-construction seam itself (`spawn_load_list` etc.) is private
/// to `src/tui/shell.rs` and exercised by `tests/unit/tui.rs`, which is
/// outside this task's `scope.target_files`; this test covers the shared
/// client-error boundary those private call sites depend on.
#[tokio::test]
async fn tui_run_search_401_yields_typed_unauthorized_with_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = server_instance(&server, "work");
    let result = crate::tui::run_search(&instance, "project = PROJ").await;

    match result {
        Err(crate::client::ClientError::Unauthorized { instance }) => {
            assert_eq!(instance, "work");
        }
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC2: search_core on a 401 prints the exact actionable re-auth message to
/// stderr and exits non-zero (rather than the 'invalid JQL'/'Error running
/// search' non-401 branches).
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn search_core_401_prints_reauth_message_and_exits_nonzero() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let instance = save_instance_for_server(&store, &server, "work");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = search_core(Some("project = PROJ"), &instance, false, &mut out, &mut err).await;

    assert_ne!(code, 0, "401 must exit non-zero");
    let err_text = output_str(&err);
    assert!(
        err_text.contains(EXPECTED_REAUTH_EN),
        "stderr must contain the exact re-auth guidance; got: {err_text}"
    );
    assert!(
        !err_text.contains("invalid JQL"),
        "a 401 must never be misclassified as invalid JQL; got: {err_text}"
    );
    server.verify().await;
}

// ---- comment_core integration tests (BDR 0014) ----

fn build_comment_response(id: &str) -> serde_json::Value {
    serde_json::json!({ "id": id })
}

async fn mount_comment_endpoint(server: &MockServer, key: &str, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(format!("/rest/api/3/issue/{key}/comment")))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

async fn received_body_text(server: &MockServer) -> String {
    let reqs = server.received_requests().await.unwrap_or_default();
    assert_eq!(reqs.len(), 1, "expected exactly 1 request; got {reqs:?}");
    String::from_utf8(reqs[0].body.clone()).expect("request body must be valid utf8")
}

/// Scenario 1: flag body + explicit key -> add_comment called once with the
/// key and verbatim body, confirmation on stdout, exit 0.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn comment_core_flag_body_explicit_key_posts_and_confirms() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    mount_comment_endpoint(
        &server,
        "PROJ-42",
        ResponseTemplate::new(201).set_body_json(build_comment_response("500")),
    )
    .await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("Deploy em homolog.".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));
    let text = output_str(&out);
    assert!(
        text.contains("PROJ-42"),
        "confirmation must name the issue key; got: {text}"
    );

    let body_text = received_body_text(&server).await;
    assert!(
        body_text.contains("Deploy em homolog."),
        "posted body must carry the flag text verbatim; got: {body_text}"
    );

    server.verify().await;
}

/// Scenario 2: piped multi-line body is passed verbatim (incl. the newline,
/// rendered as ADF hardBreak), exit 0.
#[tokio::test]
async fn comment_core_piped_multiline_body_passed_verbatim() {
    let server = MockServer::start().await;
    mount_comment_endpoint(
        &server,
        "PROJ-42",
        ResponseTemplate::new(201).set_body_json(build_comment_response("501")),
    )
    .await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Piped("Linha 1\nLinha 2".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0 on success; stderr: {}", output_str(&err));

    let body_text = received_body_text(&server).await;
    assert!(
        body_text.contains("Linha 1") && body_text.contains("Linha 2"),
        "both lines must reach the server verbatim; got: {body_text}"
    );
    assert!(
        body_text.contains("hardBreak"),
        "the newline must be preserved as an ADF hardBreak; got: {body_text}"
    );

    server.verify().await;
}

/// Scenario 3: --json success is exactly one minified line with ok:true,
/// comment_id and issue_key, and nothing else on stdout.
#[tokio::test]
async fn comment_core_json_flag_emits_single_minified_success_line() {
    let server = MockServer::start().await;
    mount_comment_endpoint(
        &server,
        "PROJ-42",
        ResponseTemplate::new(201).set_body_json(build_comment_response("777")),
    )
    .await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("ok".to_string()),
        &inst,
        true,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0; stderr: {}", output_str(&err));
    let text = output_str(&out).trim().to_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json output must be exactly 1 line: {text:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(obj["ok"], true, "ok must be true on success");
    assert_eq!(obj["comment_id"], "777", "comment_id must equal server id");
    assert_eq!(obj["issue_key"], "PROJ-42", "issue_key must equal the key");

    server.verify().await;
}

/// Scenario 4: CommentBody::None -> exit 2, 'no comment body', add_comment NOT called.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn comment_core_no_body_exits_2_and_makes_zero_requests() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    // No mock mounted for the comment endpoint — any POST would panic.

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::None,
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no body must exit 2");
    let err_text = output_str(&err);
    assert!(
        err_text.contains("no comment body"),
        "must report no comment body; got: {err_text}"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 0, "no body must make zero HTTP requests");
}

/// Edge case: a piped body that is empty after trimming is also a usage
/// error, not an empty write.
#[tokio::test]
async fn comment_core_blank_piped_body_exits_2_and_makes_zero_requests() {
    let server = MockServer::start().await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Piped("   \n  ".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "blank piped body must exit 2");
    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "blank piped body must make zero HTTP requests"
    );
}

/// Scenario 5: with no explicit key, the branch's key reaches the mock (the
/// same extraction `current_core` uses).
#[tokio::test]
async fn comment_core_branch_resolved_key_reaches_add_comment() {
    let server = MockServer::start().await;
    mount_comment_endpoint(
        &server,
        "PROJ-77",
        ResponseTemplate::new(201).set_body_json(build_comment_response("42")),
    )
    .await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        Some("feature/PROJ-77-thing"),
        CommentBody::Flag("hi".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(
        code,
        0,
        "branch-resolved key must succeed; stderr: {}",
        output_str(&err)
    );
    server.verify().await;
}

/// Scenario 6: no explicit key and a branch with no resolvable key -> exit 2,
/// zero requests.
#[tokio::test]
async fn comment_core_no_resolvable_key_exits_2_with_zero_requests() {
    let server = MockServer::start().await;
    // No mock mounted — any POST would panic.

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        Some("main"),
        CommentBody::Flag("hi".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no resolvable key must exit 2");
    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "no resolvable key must make zero HTTP requests"
    );
}

/// Scenario 6 (no-repo variant): no explicit key and no branch at all (not in
/// a git repo) -> exit 2, zero requests.
#[tokio::test]
async fn comment_core_no_branch_at_all_exits_2_with_zero_requests() {
    let server = MockServer::start().await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        None,
        CommentBody::Flag("hi".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no branch at all must exit 2");
    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 0, "must make zero HTTP requests");
}

/// Scenario 8 (Other error): a non-401 HTTP failure exits 1, no success line,
/// no false ok.
#[tokio::test]
async fn comment_core_other_error_exits_1_with_no_success_line() {
    let server = MockServer::start().await;
    mount_comment_endpoint(&server, "PROJ-42", ResponseTemplate::new(500)).await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("text".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "server error must exit 1");
    let out_text = output_str(&out);
    assert!(
        out_text.is_empty(),
        "no success line must be printed on error; got: {out_text}"
    );
    server.verify().await;
}

/// Scenario 8 (Other error, --json): the same failure emits a single
/// {"ok":false,"error":...} line, never a false success.
#[tokio::test]
async fn comment_core_other_error_json_emits_ok_false() {
    let server = MockServer::start().await;
    mount_comment_endpoint(&server, "PROJ-42", ResponseTemplate::new(500)).await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("text".to_string()),
        &inst,
        true,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1, "server error must exit 1");
    let text = output_str(&out).trim().to_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "json error output must be exactly 1 line: {text:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(obj["ok"], false, "ok must be false on failure");
    assert!(obj.get("error").is_some(), "must carry an error field");
    server.verify().await;
}

/// Scenario 7 (401): the same re-auth message the E2 tests assert appears,
/// non-zero exit, no success line.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn comment_core_401_prints_reauth_message_and_exits_nonzero() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("en");

    let server = MockServer::start().await;
    mount_comment_endpoint(&server, "PROJ-42", ResponseTemplate::new(401)).await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("text".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_ne!(code, 0, "401 must exit non-zero");
    let err_text = output_str(&err);
    assert!(
        err_text.contains(EXPECTED_REAUTH_EN),
        "stderr must contain the exact re-auth guidance; got: {err_text}"
    );
    assert!(
        output_str(&out).is_empty(),
        "no success line must be printed on 401; got: {}",
        output_str(&out)
    );
    server.verify().await;
}

/// pt-BR: the confirmation and 'no comment body' usage strings render
/// translated (existing i18n test discipline, lock held across the `.await`s).
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn comment_core_pt_br_renders_translated_confirmation_and_usage_error() {
    let _lock = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");

    let server = MockServer::start().await;
    mount_comment_endpoint(
        &server,
        "PROJ-42",
        ResponseTemplate::new(201).set_body_json(build_comment_response("900")),
    )
    .await;

    let inst = server_instance(&server, "work");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("PROJ-42"),
        None,
        CommentBody::Flag("ok".to_string()),
        &inst,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "exit 0; stderr: {}", output_str(&err));
    let text = output_str(&out);
    assert!(
        text.contains("Comentário adicionado a PROJ-42."),
        "confirmation must render in pt_BR; got: {text}"
    );

    let mut out2 = Vec::new();
    let mut err2 = Vec::new();
    let code2 = comment_core(
        Some("PROJ-1"),
        None,
        CommentBody::None,
        &inst,
        false,
        &mut out2,
        &mut err2,
    )
    .await;

    set_language("en");
    assert_eq!(code2, 2, "no body must still exit 2 in pt_BR");
    let err_text2 = output_str(&err2);
    assert!(
        err_text2.contains("corpo do comentário ausente"),
        "usage error must render in pt_BR; got: {err_text2}"
    );

    server.verify().await;
}
