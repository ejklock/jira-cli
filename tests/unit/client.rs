use super::*;
use crate::store::instances::Instance;
use crate::test_support::*;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_instance(base_url: &str) -> Instance {
    Instance {
        name: "test-instance".to_string(),
        base_url: base_url.to_string(),
        email: "user@example.com".to_string(),
        token: "test-api-token".to_string(),
        account_id: None,
    }
}

fn build_issue_payload() -> serde_json::Value {
    crate::test_support::build_issue_payload(IssuePayloadOptions {
        status_description: "This issue is being worked on",
        issuetype_description: "A problem or error",
        assignee_account_id: "5b10a2844c20165700ede21g",
        assignee_display_name: "Alice Example",
        assignee_email: Some("alice@example.com"),
        reporter: None,
        comment_author_account_id: "aaa",
        comment_collection_self: Some(
            "https://example.atlassian.net/rest/api/3/issue/10001/comment",
        ),
        attachments: Some(serde_json::json!([
            {
                "id": "200",
                "filename": "screenshot.png",
                "content": "https://example.atlassian.net/secure/attachment/200/screenshot.png",
                "mimeType": "image/png",
                "size": 2048
            },
            {
                "id": "201",
                "filename": "notes.txt",
                "content": "https://example.atlassian.net/secure/attachment/201/notes.txt"
            }
        ])),
    })
}

#[tokio::test]
async fn get_issue_returns_mapped_issue_with_all_curated_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(issue.key, "PROJ-123");
    assert_eq!(issue.summary, "Fix the login bug");
    assert_eq!(issue.status, "In Progress");
    assert_eq!(issue.status_category.as_deref(), Some("indeterminate"));
    assert_eq!(issue.issue_type, "Bug");
    assert_eq!(issue.priority.as_deref(), Some("High"));
    assert!(issue.created.is_some(), "created must be mapped");
    assert!(issue.updated.is_some(), "updated must be mapped");

    let assignee = issue.assignee.expect("assignee must be mapped");
    assert_eq!(assignee.display_name, "Alice Example");
    assert_eq!(
        assignee.account_id.as_deref(),
        Some("5b10a2844c20165700ede21g")
    );

    assert!(
        issue
            .description
            .as_deref()
            .unwrap_or("")
            .contains("Login fails"),
        "ADF description should be flattened to plain text"
    );

    assert_eq!(issue.comments.len(), 1);
    assert_eq!(issue.comments[0].body, "Reproduced on v2.1.");
    assert_eq!(issue.comments[0].author.as_deref(), Some("Bob Dev"));

    assert_eq!(
        issue.attachments.len(),
        2,
        "both attachments must be mapped"
    );
    assert_eq!(issue.attachments[0].filename, "screenshot.png");
    assert_eq!(
        issue.attachments[0].url,
        "https://example.atlassian.net/secure/attachment/200/screenshot.png"
    );
    assert_eq!(issue.attachments[0].mime_type.as_deref(), Some("image/png"));
    assert_eq!(issue.attachments[0].size, Some(2048));
    assert_eq!(issue.attachments[1].filename, "notes.txt");
    assert_eq!(
        issue.attachments[1].url,
        "https://example.atlassian.net/secure/attachment/201/notes.txt"
    );
    assert_eq!(
        issue.attachments[1].mime_type, None,
        "mimeType absent must map to None"
    );
    assert_eq!(
        issue.attachments[1].size, None,
        "size absent must map to None"
    );

    server.verify().await;
}

/// AC1: a payload with no `fields.attachment` key must map to an empty vec,
/// never an error.
#[tokio::test]
async fn get_issue_maps_attachments_to_empty_vec_when_field_absent() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]
        .as_object_mut()
        .unwrap()
        .remove("attachment");
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert!(
        issue.attachments.is_empty(),
        "absent attachment field must map to an empty vec"
    );
    server.verify().await;
}

/// AC1: `fields.attachment: null` must map to an empty vec, never an error.
#[tokio::test]
async fn get_issue_maps_attachments_to_empty_vec_when_field_is_null() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]["attachment"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert!(
        issue.attachments.is_empty(),
        "null attachment field must map to an empty vec"
    );
    server.verify().await;
}

/// AC1: a malformed entry (missing filename) among otherwise valid entries is
/// skipped — never an error or panic — while valid entries still parse.
#[tokio::test]
async fn get_issue_skips_malformed_attachment_entry_and_keeps_valid_ones() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]["attachment"] = serde_json::json!([
        {
            "id": "200",
            "filename": "screenshot.png",
            "content": "https://example.atlassian.net/secure/attachment/200/screenshot.png",
            "mimeType": "image/png",
            "size": 2048
        },
        {
            "id": "202",
            "content": "https://example.atlassian.net/secure/attachment/202/missing-filename"
        },
        {
            "id": "203",
            "filename": "no-content.txt"
        }
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(
        issue.attachments.len(),
        1,
        "malformed entries (missing filename or content) must be skipped"
    );
    assert_eq!(issue.attachments[0].filename, "screenshot.png");
    server.verify().await;
}

#[tokio::test]
async fn get_issue_maps_duedate_when_present() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]["duedate"] = serde_json::json!("2026-07-15");
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(issue.duedate.as_deref(), Some("2026-07-15"));
    server.verify().await;
}

#[tokio::test]
async fn get_issue_maps_duedate_to_none_when_absent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(issue.duedate, None, "no raw duedate field must map to None");
    server.verify().await;
}

#[tokio::test]
async fn get_issue_null_assignee_maps_to_none() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]["assignee"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-1").await.unwrap();

    assert_eq!(issue.assignee, None);
    server.verify().await;
}

#[tokio::test]
async fn get_issue_404_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-404"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.get_issue("PROJ-404").await;

    assert!(result.is_err(), "404 must propagate as Err");
    server.verify().await;
}

/// AC1: a 401 on issue GET must map to the typed `ClientError::Unauthorized`
/// carrying this client's instance name — matched by type, never by message.
#[tokio::test]
async fn get_issue_401_maps_to_typed_unauthorized_with_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-401"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.get_issue("PROJ-401").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => {
            assert_eq!(instance, "test-instance");
        }
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1: a 403 on issue GET must NOT map to Unauthorized — it stays `Other`.
#[tokio::test]
async fn get_issue_403_does_not_map_to_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-403"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.get_issue("PROJ-403").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 403, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1: a 500 on issue GET must NOT map to Unauthorized — it stays `Other`.
#[tokio::test]
async fn get_issue_500_does_not_map_to_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-500"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.get_issue("PROJ-500").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 500, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1: a 401 on search must also map to the typed `ClientError::Unauthorized`
/// carrying this client's instance name.
#[tokio::test]
async fn search_401_maps_to_typed_unauthorized_with_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await;

    match result {
        Err(ClientError::Unauthorized { instance }) => {
            assert_eq!(instance, "test-instance");
        }
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1: a 403 on search must NOT map to Unauthorized — it stays `Other`.
#[tokio::test]
async fn search_403_does_not_map_to_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 403, got: {other:?}"),
    }
    server.verify().await;
}

#[tokio::test]
async fn myself_returns_account_id_and_display_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let me = client.myself().await.unwrap();

    assert_eq!(me.account_id, "5b10a2844c20165700ede21g");
    assert_eq!(me.display_name, "Alice Example");
    server.verify().await;
}

#[tokio::test]
async fn myself_attaches_basic_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header(
            "Authorization",
            "Basic dXNlckBleGFtcGxlLmNvbTp0ZXN0LWFwaS10b2tlbg==",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    client.myself().await.unwrap();
    server.verify().await;
}

/// Host isolation: the client is constructed from instance.base_url and only sends requests there.
/// A second mock server receives zero requests even when it is listening.
#[tokio::test]
async fn host_isolation_requests_go_only_to_instance_host() {
    let instance_server = MockServer::start().await;
    let other_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .expect(1)
        .mount(&instance_server)
        .await;

    // The other server must receive zero requests.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&other_server)
        .await;

    let instance = make_instance(&instance_server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    client.myself().await.unwrap();

    instance_server.verify().await;
    other_server.verify().await;
}

/// Authorization header must not be sent to a different host.
/// Since gouqi constructs its reqwest client pinned to the instance host, any
/// request from this client can ONLY reach instance_server by construction.
/// We assert that other_server received no Authorization header (zero requests is stronger).
#[tokio::test]
async fn host_isolation_no_authorization_sent_to_other_host() {
    let instance_server = MockServer::start().await;
    let other_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_myself_payload()))
        .mount(&instance_server)
        .await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&other_server)
        .await;

    let instance = make_instance(&instance_server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    client.myself().await.unwrap();

    let reqs = other_server.received_requests().await.unwrap();
    assert!(
        reqs.is_empty(),
        "other host must receive zero requests, not: {reqs:?}"
    );
    let any_with_auth = reqs.iter().any(|r| r.headers.contains_key("authorization"));
    assert!(
        !any_with_auth,
        "Authorization must never be sent to a different host"
    );
}

#[tokio::test]
async fn search_returns_issue_rows_with_mapped_fields() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [
            {
                "id": "10001",
                "key": "PROJ-1",
                "self": "https://example.atlassian.net/rest/api/3/issue/10001",
                "fields": {
                    "summary": "First issue",
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
                        "displayName": "Alice",
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
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await.unwrap();

    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "PROJ-1");
    assert_eq!(result.issues[0].summary, "First issue");
    assert_eq!(result.issues[0].status, "Open");
    assert_eq!(result.issues[0].assignee.as_deref(), Some("Alice"));
    assert!(result.is_last_page);
    assert_eq!(
        result.next_page_token, None,
        "last page must map to no next_page_token"
    );
    assert_eq!(
        result.issues[0].duedate, None,
        "a search payload with no duedate field must map to None"
    );
    assert_eq!(
        result.issues[0].project, None,
        "a search payload with no project field must map to None"
    );
    server.verify().await;
}

/// A single search-result issue with `fields.duedate`/`fields.project` set, so
/// `map_gouqi_search_results` (issue 0031 / D2) has something to extract.
fn build_search_issue_with_due_and_project(
    key: &str,
    duedate: &str,
    project_key: &str,
    project_name: Option<&str>,
) -> serde_json::Value {
    let mut project = serde_json::json!({ "key": project_key, "self": "" });
    if let Some(name) = project_name {
        project["name"] = serde_json::json!(name);
    }
    serde_json::json!({
        "id": "10001",
        "key": key,
        "self": "https://example.atlassian.net/rest/api/3/issue/10001",
        "fields": {
            "summary": "Card issue",
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
            "duedate": duedate,
            "project": project
        }
    })
}

#[tokio::test]
async fn search_maps_duedate_and_project_when_present() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [build_search_issue_with_due_and_project(
            "PROJ-1",
            "2026-07-15",
            "PROJ",
            Some("Proj Display"),
        )],
        "isLast": true,
        "nextPageToken": null
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await.unwrap();

    assert_eq!(result.issues[0].duedate.as_deref(), Some("2026-07-15"));
    assert_eq!(
        result.issues[0].project.as_deref(),
        Some("Proj Display"),
        "project name must be preferred over the project key"
    );
    server.verify().await;
}

#[tokio::test]
async fn search_maps_project_key_when_project_name_is_absent() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [build_search_issue_with_due_and_project(
            "PROJ-2",
            "2026-07-15",
            "PROJ",
            None,
        )],
        "isLast": true,
        "nextPageToken": null
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await.unwrap();

    assert_eq!(
        result.issues[0].project.as_deref(),
        Some("PROJ"),
        "project must fall back to the project key when the name is absent"
    );
    server.verify().await;
}

/// Cached search-result JSON written before `IssueRow` gained `duedate`/
/// `project` (issue 0031) must still deserialize — `#[serde(default)]`
/// back-compat, no wiremock needed.
#[test]
fn issue_row_pre_field_cached_json_deserializes_with_none_duedate_and_project() {
    let pre_field_snapshot = serde_json::json!({
        "key": "PROJ-9",
        "issue_type": "Task",
        "summary": "Cached before D2",
        "status": "Open",
        "assignee": "Alice"
    });

    let row: crate::models::IssueRow = serde_json::from_value(pre_field_snapshot).unwrap();

    assert_eq!(row.key, "PROJ-9");
    assert_eq!(
        row.duedate, None,
        "pre-field cache must default duedate to None"
    );
    assert_eq!(
        row.project, None,
        "pre-field cache must default project to None"
    );
}

/// AC2: cached `Issue` JSON written before `attachments` existed (pre-B4)
/// must still deserialize, defaulting to an empty vec — the same
/// `#[serde(default)]` back-compat pattern as `duedate`/`comments`.
#[test]
fn issue_pre_attachments_cached_json_deserializes_with_empty_attachments() {
    let pre_field_snapshot = serde_json::json!({
        "key": "PROJ-9",
        "summary": "Cached before B4",
        "status": "Open",
        "status_category": null,
        "issue_type": "Task",
        "assignee": null,
        "reporter": null,
        "priority": null,
        "created": null,
        "updated": null,
        "description": null,
        "comments": []
    });

    let issue: crate::models::Issue = serde_json::from_value(pre_field_snapshot).unwrap();

    assert!(
        issue.attachments.is_empty(),
        "pre-B4 cache without the attachments key must default to an empty vec"
    );
}

/// AC2: a populated `attachments` vec survives a serialize/deserialize
/// round-trip — the cache write/read guarantee (BDR 0012 S1).
#[test]
fn issue_attachments_roundtrip_through_serde() {
    let issue = crate::models::Issue {
        summary: "Has attachments".to_string(),
        status_category: None,
        assignee: None,
        reporter: None,
        priority: None,
        created: None,
        updated: None,
        description: None,
        attachments: vec![crate::test_support::attachment(
            "screenshot.png",
            "https://example.atlassian.net/secure/attachment/200/screenshot.png",
            Some("image/png"),
            Some(2048),
        )],
        ..crate::test_support::issue("PROJ-10")
    };

    let serialized = serde_json::to_value(&issue).unwrap();
    let deserialized: crate::models::Issue = serde_json::from_value(serialized).unwrap();

    assert_eq!(
        deserialized.attachments, issue.attachments,
        "a populated attachments vec must round-trip unchanged"
    );
}

#[tokio::test]
async fn search_maps_next_page_token_when_server_returns_one() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [],
        "isLast": false,
        "nextPageToken": "TOK2"
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.search("project = PROJ", 50).await.unwrap();

    assert_eq!(
        result.next_page_token.as_deref(),
        Some("TOK2"),
        "a non-last page must carry the server's next_page_token through"
    );
    assert!(!result.is_last_page);
    server.verify().await;
}

#[tokio::test]
async fn search_page_issues_next_page_token_param_and_maps_following_token() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [
            {
                "id": "10002",
                "key": "PROJ-2",
                "self": "https://example.atlassian.net/rest/api/3/issue/10002",
                "fields": {
                    "summary": "Second page issue",
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
                    }
                }
            }
        ],
        "isLast": false,
        "nextPageToken": "TOK3"
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .and(query_param("nextPageToken", "TOK1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client
        .search_page("project = PROJ", 50, "TOK1")
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "PROJ-2");
    assert_eq!(
        result.next_page_token.as_deref(),
        Some("TOK3"),
        "search_page must map the following page's token through"
    );
    assert!(!result.is_last_page);
    server.verify().await;
}

#[tokio::test]
async fn search_page_maps_none_token_on_last_page() {
    let server = MockServer::start().await;
    let search_payload = serde_json::json!({
        "issues": [],
        "isLast": true,
        "nextPageToken": null
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/search/jql"))
        .and(query_param("nextPageToken", "TOK1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client
        .search_page("project = PROJ", 50, "TOK1")
        .await
        .unwrap();

    assert_eq!(result.next_page_token, None);
    assert!(result.is_last_page);
    server.verify().await;
}

/// AC1: list_projects issues GET /project/search with maxResults=100 and
/// maps `values[]` to `ProjectRow{key, name}`, preserving order.
#[tokio::test]
async fn list_projects_issues_max_results_param_and_maps_values_in_order() {
    let server = MockServer::start().await;
    let payload = serde_json::json!({
        "values": [
            { "key": "PROJ", "name": "Project One" },
            { "key": "OTHER", "name": "Project Two" }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await.unwrap();

    assert_eq!(
        result,
        vec![
            crate::test_support::project_row("PROJ", "Project One"),
            crate::test_support::project_row("OTHER", "Project Two"),
        ]
    );
    server.verify().await;
}

/// AC2: an entry missing `key` or `name` is skipped, while valid entries survive.
#[tokio::test]
async fn list_projects_skips_entries_missing_key_or_name() {
    let server = MockServer::start().await;
    let payload = serde_json::json!({
        "values": [
            { "key": "PROJ", "name": "Project One" },
            { "name": "Missing Key" },
            { "key": "NONAME" },
            { "key": "OTHER", "name": "Project Two" }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await.unwrap();

    assert_eq!(
        result,
        vec![
            crate::test_support::project_row("PROJ", "Project One"),
            crate::test_support::project_row("OTHER", "Project Two"),
        ],
        "entries missing key or name must be skipped, not error or panic"
    );
    server.verify().await;
}

/// AC2: `values` absent or `null` yields an empty vec, never an error.
#[tokio::test]
async fn list_projects_absent_or_null_values_yields_empty_vec() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await.unwrap();

    assert_eq!(result, Vec::new(), "absent values must map to an empty vec");
    server.verify().await;
}

#[tokio::test]
async fn list_projects_null_values_yields_empty_vec() {
    let server = MockServer::start().await;
    let payload = serde_json::json!({ "values": null });

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await.unwrap();

    assert_eq!(result, Vec::new(), "null values must map to an empty vec");
    server.verify().await;
}

/// AC3: a 401 on project/search must map to the same typed `ClientError::Unauthorized`
/// the other client calls produce, carrying this client's instance name.
#[tokio::test]
async fn list_projects_401_maps_to_typed_unauthorized_with_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await;

    match result {
        Err(ClientError::Unauthorized { instance }) => {
            assert_eq!(instance, "test-instance");
        }
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC3: a 5xx on project/search maps to the standard `ClientError::Other` variant,
/// not Unauthorized. A real server error commonly returns a non-JSON body (a
/// proxy error page, not an API payload); `serde_json::Value` parsing of that
/// body is what surfaces the error here — the same implicit mechanism
/// `get_issue`/`search`'s 500 tests rely on for their typed response bodies.
#[tokio::test]
async fn list_projects_500_does_not_map_to_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_projects().await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 500, got: {other:?}"),
    }
    server.verify().await;
}

// --- plain_text_to_adf (AC2) ---

#[test]
fn plain_text_to_adf_single_line_yields_one_text_node() {
    let doc = plain_text_to_adf("hello world");
    assert_eq!(
        doc,
        serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "hello world"}]
            }]
        })
    );
}

#[test]
fn plain_text_to_adf_multi_line_interleaves_hard_breaks() {
    let doc = plain_text_to_adf("a\nb");
    assert_eq!(
        doc,
        serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [
                    {"type": "text", "text": "a"},
                    {"type": "hardBreak"},
                    {"type": "text", "text": "b"}
                ]
            }]
        })
    );
}

#[test]
fn plain_text_to_adf_empty_input_yields_valid_doc_without_empty_text_nodes() {
    let doc = plain_text_to_adf("");
    assert_eq!(
        doc,
        serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{"type": "paragraph", "content": []}]
        })
    );
}

#[test]
fn plain_text_to_adf_leading_and_trailing_newlines_never_emit_empty_text_nodes() {
    let doc = plain_text_to_adf("\na\n");
    assert_eq!(
        doc,
        serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [
                    {"type": "hardBreak"},
                    {"type": "text", "text": "a"},
                    {"type": "hardBreak"}
                ]
            }]
        })
    );
}

// --- add_comment / update_comment / delete_comment (AC1, AC3) ---

fn expected_comment_adf_body(text: &str) -> serde_json::Value {
    serde_json::json!({ "body": plain_text_to_adf(text) })
}

/// AC1: add_comment POSTs to the literal v3 path with the ADF-wrapped body
/// and maps the response into `CommentWriteResult`.
#[tokio::test]
async fn add_comment_posts_v3_path_with_adf_body_and_returns_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .and(body_json(expected_comment_adf_body(
            "Reproduced on staging.",
        )))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": "10001"})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client
        .add_comment("PROJ-1", "Reproduced on staging.")
        .await
        .unwrap();

    assert_eq!(result.id, "10001");
    server.verify().await;
}

/// AC1: a 401 on add_comment maps to the typed `ClientError::Unauthorized`.
#[tokio::test]
async fn add_comment_401_maps_to_typed_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.add_comment("PROJ-1", "text").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test-instance"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC3: a 400 on add_comment must never be a false Ok — it stays `Other`.
#[tokio::test]
async fn add_comment_400_does_not_map_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.add_comment("PROJ-1", "text").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 400, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1: update_comment PUTs to the literal v3 path (the received-path assert
/// IS the dot-segment normalization's falsifiability guard — ADR 0022 addendum).
#[tokio::test]
async fn update_comment_puts_v3_path_with_adf_body_and_returns_id() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .and(body_json(expected_comment_adf_body("Updated text.")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "10001"})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client
        .update_comment("PROJ-1", "10001", "Updated text.")
        .await
        .unwrap();

    assert_eq!(result.id, "10001");
    server.verify().await;
}

/// AC3: a 403 on update_comment must never be a false Ok — it stays `Other`.
#[tokio::test]
async fn update_comment_403_does_not_map_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.update_comment("PROJ-1", "10001", "text").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 403, got: {other:?}"),
    }
    server.verify().await;
}

/// AC1/AC3: delete_comment DELETEs to the literal v3 path and maps a 204
/// empty body to `Ok(())` — the received-path assert is the same dot-segment
/// falsifiability guard as `update_comment`'s.
#[tokio::test]
async fn delete_comment_deletes_v3_path_and_maps_204_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.delete_comment("PROJ-1", "10001").await;

    assert!(
        result.is_ok(),
        "204 empty body must map to Ok(()): {result:?}"
    );
    server.verify().await;
}

/// AC3: a 401 on delete_comment maps to the typed `ClientError::Unauthorized`.
#[tokio::test]
async fn delete_comment_401_maps_to_typed_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.delete_comment("PROJ-1", "10001").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test-instance"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC3: a 500 on delete_comment must never be a false Ok — it stays `Other`.
#[tokio::test]
async fn delete_comment_500_does_not_map_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/PROJ-1/comment/10001"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.delete_comment("PROJ-1", "10001").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 500, got: {other:?}"),
    }
    server.verify().await;
}

// --- comment author_account_id read regression (AC4) ---

/// AC4: a comment payload with `author.accountId` maps to `Some`.
#[tokio::test]
async fn get_issue_comment_author_account_id_maps_to_some_when_present() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_issue_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(
        issue.comments[0].author_account_id.as_deref(),
        Some("aaa"),
        "comment author.accountId must map to author_account_id"
    );
    server.verify().await;
}

/// AC4: a comment payload with no `author` at all maps `author_account_id`
/// to `None`, never an error.
#[tokio::test]
async fn get_issue_comment_author_account_id_maps_to_none_when_author_absent() {
    let server = MockServer::start().await;
    let mut payload = build_issue_payload();
    payload["fields"]["comment"]["comments"][0]
        .as_object_mut()
        .unwrap()
        .remove("author");
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let issue = client.get_issue("PROJ-123").await.unwrap();

    assert_eq!(
        issue.comments[0].author_account_id, None,
        "absent author must map author_account_id to None"
    );
    server.verify().await;
}

// --- list_transitions / transition_issue (ADR 0027, BDR 0018) ---

fn build_transitions_payload() -> serde_json::Value {
    serde_json::json!({
        "transitions": [
            {
                "id": "11",
                "name": "Start Progress",
                "to": { "id": "3", "name": "In Progress" },
                "fields": {}
            },
            {
                "id": "31",
                "name": "Done",
                "to": { "id": "10001", "name": "Done" },
                "fields": {
                    "resolution": { "required": true, "name": "Resolution" }
                }
            }
        ]
    })
}

/// AC1: list_transitions GETs the literal v3 path with `expand=transitions.fields`
/// and parses a MIXED payload (one field-free, one field-requiring) into both
/// rows exactly, including `to_status` and `requires_fields`.
#[tokio::test]
async fn list_transitions_gets_expand_and_parses_mixed_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .and(query_param("expand", "transitions.fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(build_transitions_payload()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let transitions = client.list_transitions("PROJ-1").await.unwrap();

    assert_eq!(
        transitions,
        vec![
            crate::models::Transition {
                id: "11".to_string(),
                name: "Start Progress".to_string(),
                to_status: "In Progress".to_string(),
                requires_fields: false,
            },
            crate::models::Transition {
                id: "31".to_string(),
                name: "Done".to_string(),
                to_status: "Done".to_string(),
                requires_fields: true,
            },
        ]
    );
    server.verify().await;
}

/// AC1 edge case: `transitions` absent or non-array must yield an empty vec,
/// never an error.
#[tokio::test]
async fn list_transitions_absent_transitions_key_yields_empty_vec() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let transitions = client.list_transitions("PROJ-1").await.unwrap();

    assert_eq!(
        transitions,
        Vec::new(),
        "absent transitions key must map to an empty vec"
    );
    server.verify().await;
}

/// AC1 edge case: an entry missing `id` or `name` is skipped, valid entries survive.
#[tokio::test]
async fn list_transitions_skips_entries_missing_id_or_name() {
    let server = MockServer::start().await;
    let payload = serde_json::json!({
        "transitions": [
            { "id": "11", "name": "Start Progress", "to": { "name": "In Progress" }, "fields": {} },
            { "name": "Missing Id", "to": { "name": "Done" }, "fields": {} },
            { "id": "99", "to": { "name": "Done" }, "fields": {} }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let transitions = client.list_transitions("PROJ-1").await.unwrap();

    assert_eq!(
        transitions.len(),
        1,
        "entries missing id/name must be skipped"
    );
    assert_eq!(transitions[0].id, "11");
    server.verify().await;
}

/// AC3: a 401 on list_transitions maps to the typed `ClientError::Unauthorized`
/// carrying this client's instance name.
#[tokio::test]
async fn list_transitions_401_maps_to_typed_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.list_transitions("PROJ-1").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test-instance"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC2: transition_issue POSTs the literal v3 path with body exactly
/// `{"transition":{"id":"<id>"}}` and maps a 204 empty body to `Ok(())`.
#[tokio::test]
async fn transition_issue_posts_body_and_maps_204_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .and(body_json(
            serde_json::json!({ "transition": { "id": "31" } }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.transition_issue("PROJ-1", "31").await;

    assert!(
        result.is_ok(),
        "204 empty body must map to Ok(()): {result:?}"
    );
    server.verify().await;
}

/// AC3: a 401 on transition_issue maps to the typed `ClientError::Unauthorized`.
#[tokio::test]
async fn transition_issue_401_maps_to_typed_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.transition_issue("PROJ-1", "31").await;

    match result {
        Err(ClientError::Unauthorized { instance }) => assert_eq!(instance, "test-instance"),
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC3: a 400 on transition_issue must never be a false Ok — it stays `Other`.
#[tokio::test]
async fn transition_issue_400_does_not_map_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let result = client.transition_issue("PROJ-1", "31").await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 400, got: {other:?}"),
    }
    server.verify().await;
}

// --- download_attachment (ADR 0029 §1, BDR 0020 S1-S3) ---

/// AC1: a same-origin GET returns the served body bytes verbatim.
#[tokio::test]
async fn download_attachment_same_origin_returns_body_bytes_verbatim() {
    let server = MockServer::start().await;
    let body = b"\x89PNG-fake-binary-content".to_vec();
    Mock::given(method("GET"))
        .and(path("/secure/attachment/200/screenshot.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let url = format!("{}/secure/attachment/200/screenshot.png", server.uri());
    let bytes = client.download_attachment(&url).await.unwrap();

    assert_eq!(bytes.as_ref(), body.as_slice());
    server.verify().await;
}

/// AC1: the same-origin GET carries the instance's Basic-auth credentials.
#[tokio::test]
async fn download_attachment_attaches_basic_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secure/attachment/200/screenshot.png"))
        .and(header(
            "Authorization",
            "Basic dXNlckBleGFtcGxlLmNvbTp0ZXN0LWFwaS10b2tlbg==",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let url = format!("{}/secure/attachment/200/screenshot.png", server.uri());
    client.download_attachment(&url).await.unwrap();

    server.verify().await;
}

/// AC2: a cross-origin url is rejected with a typed error and the
/// cross-origin server records ZERO requests — the same-origin guard runs
/// before any network call.
#[tokio::test]
async fn download_attachment_cross_origin_url_rejected_with_zero_requests() {
    let instance_server = MockServer::start().await;
    let other_server = MockServer::start().await;

    let instance = make_instance(&instance_server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let cross_origin_url = format!(
        "{}/secure/attachment/200/screenshot.png",
        other_server.uri()
    );
    let result = client.download_attachment(&cross_origin_url).await;

    assert!(
        result.is_err(),
        "a cross-origin url must be rejected: {result:?}"
    );
    let reqs = other_server.received_requests().await.unwrap();
    assert!(
        reqs.is_empty(),
        "the cross-origin server must receive zero requests, not: {reqs:?}"
    );
}

/// AC3: a same-origin content url that responds 401 surfaces the typed
/// `ClientError::Unauthorized` carrying the instance name.
#[tokio::test]
async fn download_attachment_401_maps_to_typed_unauthorized_with_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secure/attachment/200/screenshot.png"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let url = format!("{}/secure/attachment/200/screenshot.png", server.uri());
    let result = client.download_attachment(&url).await;

    match result {
        Err(ClientError::Unauthorized { instance }) => {
            assert_eq!(instance, "test-instance");
        }
        other => panic!("expected ClientError::Unauthorized, got: {other:?}"),
    }
    server.verify().await;
}

/// AC2: a non-401, non-2xx same-origin response must never be a false Ok —
/// it stays `Other`.
#[tokio::test]
async fn download_attachment_404_does_not_map_to_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secure/attachment/missing.png"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let instance = make_instance(&server.uri());
    let client = GouqiJiraClient::new(&instance).unwrap();
    let url = format!("{}/secure/attachment/missing.png", server.uri());
    let result = client.download_attachment(&url).await;

    match result {
        Err(ClientError::Other(_)) => {}
        other => panic!("expected ClientError::Other for 404, got: {other:?}"),
    }
    server.verify().await;
}

/// Pure unit test for the `same_origin` helper: same scheme+host+port is
/// true, and differing scheme, host, or port each flip it to false.
#[test]
fn same_origin_compares_scheme_host_and_port() {
    let base = Url::parse("https://example.atlassian.net").unwrap();

    assert!(same_origin(
        &Url::parse("https://example.atlassian.net/secure/attachment/1").unwrap(),
        &base
    ));
    assert!(
        !same_origin(
            &Url::parse("http://example.atlassian.net/x").unwrap(),
            &base
        ),
        "differing scheme must not be same-origin"
    );
    assert!(
        !same_origin(&Url::parse("https://evil.example.net/x").unwrap(), &base),
        "differing host must not be same-origin"
    );
    assert!(
        !same_origin(
            &Url::parse("https://example.atlassian.net:8443/x").unwrap(),
            &base
        ),
        "differing port must not be same-origin"
    );
}
