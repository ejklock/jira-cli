use super::*;
use crate::store::instances::Instance;
use wiremock::matchers::{header, method, path, query_param};
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
    serde_json::json!({
        "id": "10001",
        "key": "PROJ-123",
        "self": "https://example.atlassian.net/rest/api/3/issue/10001",
        "fields": {
            "summary": "Fix the login bug",
            "status": {
                "id": "3",
                "name": "In Progress",
                "description": "This issue is being worked on",
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
                "description": "A problem or error",
                "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/bug.png",
                "self": "https://example.atlassian.net/rest/api/3/issuetype/10002",
                "subtask": false
            },
            "assignee": {
                "accountId": "5b10a2844c20165700ede21g",
                "displayName": "Alice Example",
                "emailAddress": "alice@example.com",
                "active": true,
                "self": "https://example.atlassian.net/rest/api/3/user?accountId=5b10a2844c20165700ede21g",
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
                "content": [
                    {
                        "type": "paragraph",
                        "content": [
                            {
                                "type": "text",
                                "text": "Login fails when MFA is enabled."
                            }
                        ]
                    }
                ]
            },
            "comment": {
                "comments": [
                    {
                        "id": "100",
                        "self": "https://example.atlassian.net/rest/api/3/issue/10001/comment/100",
                        "author": {
                            "accountId": "aaa",
                            "displayName": "Bob Dev",
                            "active": true,
                            "self": "https://example.atlassian.net/rest/api/3/user?accountId=aaa",
                            "avatarUrls": {}
                        },
                        "body": "Reproduced on v2.1.",
                        "created": "2026-06-29T10:00:00.000+0000",
                        "updated": "2026-06-29T10:00:00.000+0000"
                    }
                ],
                "self": "https://example.atlassian.net/rest/api/3/issue/10001/comment",
                "maxResults": 1,
                "total": 1,
                "startAt": 0
            }
        }
    })
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
