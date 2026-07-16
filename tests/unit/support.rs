use crate::models::{Attachment, Issue, IssueAssignee, IssueComment, ProjectRow};

// --- ADF fixture builders (shared by render.rs and tui.rs ADF/description/comment tests) ---
//
// These assemble ADF-JSON via `serde_json::json!` instead of repeating the
// full `{"type":"doc","version":1,"content":[...]}` scaffolding as string
// literals in every test body.

pub(crate) fn doc(content: Vec<serde_json::Value>) -> String {
    serde_json::json!({"type": "doc", "version": 1, "content": content}).to_string()
}

pub(crate) fn paragraph(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "paragraph", "content": content})
}

pub(crate) fn heading(level: u64, content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "heading", "attrs": {"level": level}, "content": content})
}

pub(crate) fn code_block(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "codeBlock", "content": content})
}

pub(crate) fn blockquote(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "blockquote", "content": content})
}

pub(crate) fn panel(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "panel", "attrs": {"panelType": "info"}, "content": content})
}

pub(crate) fn rule_block() -> serde_json::Value {
    serde_json::json!({"type": "rule"})
}

pub(crate) fn table(rows: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "table", "content": rows})
}

pub(crate) fn table_row(cells: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "tableRow", "content": cells})
}

pub(crate) fn table_header(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "tableHeader", "content": content})
}

pub(crate) fn table_cell(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "tableCell", "content": content})
}

pub(crate) fn bullet_list(items: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "bulletList", "content": items})
}

pub(crate) fn ordered_list(items: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "orderedList", "content": items})
}

pub(crate) fn list_item(content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "listItem", "content": content})
}

pub(crate) fn text(value: &str) -> serde_json::Value {
    serde_json::json!({"type": "text", "text": value})
}

pub(crate) fn hard_break() -> serde_json::Value {
    serde_json::json!({"type": "hardBreak"})
}

pub(crate) fn custom_node(node_type: &str, content: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": node_type, "content": content})
}

pub(crate) fn mark(mark_type: &str) -> serde_json::Value {
    serde_json::json!({"type": mark_type})
}

pub(crate) fn link_mark(href: &str) -> serde_json::Value {
    serde_json::json!({"type": "link", "attrs": {"href": href}})
}

pub(crate) fn marked_text(value: &str, marks: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": "text", "text": value, "marks": marks})
}

pub(crate) fn plain_paragraph(value: &str) -> String {
    doc(vec![paragraph(vec![text(value)])])
}

pub(crate) fn marked_paragraph(value: &str, marks: Vec<serde_json::Value>) -> String {
    doc(vec![paragraph(vec![marked_text(value, marks)])])
}

/// Compute a `"YYYY-MM-DD"` due date `days` away from the actual current date,
/// using the same civil-date extraction `render_issue_human`/`view_detail` use
/// internally (`crate::store::secs_to_utc_parts`), so the expected relative
/// bucket (e.g. "in 3 days") is deterministic regardless of when the test runs.
pub(crate) fn duedate_offset_from_today(days: i64) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let target_secs = (now_secs + days * 86_400).max(0) as u64;
    let (year, month, day, _, _, _) = crate::store::secs_to_utc_parts(target_secs);
    format!("{year:04}-{month:02}-{day:02}")
}

// --- Issue fixture builders ---

pub(crate) fn assignee(display_name: &str, account_id: Option<&str>) -> IssueAssignee {
    IssueAssignee {
        display_name: display_name.to_owned(),
        account_id: account_id.map(str::to_owned),
    }
}

pub(crate) fn comment(
    id: Option<&str>,
    author: Option<&str>,
    body: &str,
    created: Option<&str>,
    updated: Option<&str>,
) -> IssueComment {
    IssueComment {
        id: id.map(str::to_owned),
        author: author.map(str::to_owned),
        author_account_id: None,
        body: body.to_owned(),
        created: created.map(str::to_owned),
        updated: updated.map(str::to_owned),
    }
}

pub(crate) fn project_row(key: &str, name: &str) -> ProjectRow {
    ProjectRow {
        key: key.to_owned(),
        name: name.to_owned(),
    }
}

pub(crate) fn attachment(
    filename: &str,
    url: &str,
    mime_type: Option<&str>,
    size: Option<u64>,
) -> Attachment {
    Attachment {
        filename: filename.to_owned(),
        url: url.to_owned(),
        mime_type: mime_type.map(str::to_owned),
        size,
    }
}

/// A neutral `Issue` fixture with sane defaults covering every field. Tests
/// override only the field(s) they assert via `Issue { field: value, ..issue(key) }`,
/// so a new `Issue` field is added once, here, instead of in every test file.
pub(crate) fn issue(key: &str) -> Issue {
    Issue {
        key: key.to_owned(),
        summary: "A neutral issue summary".to_owned(),
        status: "Open".to_owned(),
        status_category: Some("new".to_owned()),
        issue_type: "Task".to_owned(),
        assignee: Some(assignee("Jane Doe", Some("acc-neutral"))),
        reporter: Some(assignee("John Reporter", Some("acc-reporter"))),
        priority: Some("Medium".to_owned()),
        created: Some("2026-01-01T00:00:00.000+0000".to_owned()),
        updated: Some("2026-01-02T00:00:00.000+0000".to_owned()),
        duedate: None,
        description: Some(plain_paragraph("A neutral issue description.")),
        comments: vec![],
        attachments: vec![],
    }
}

// --- HTTP fixture payload builders (formerly duplicated between
// tests/unit/client.rs and tests/unit/commands.rs `get_issue`/`get_core`
// tests) ---

/// Every field where `client.rs`'s and `commands.rs`'s issue payload fixtures
/// diverge. Each call site supplies its own current values so
/// `build_issue_payload` reproduces its exact prior JSON byte-for-byte.
pub(crate) struct IssuePayloadOptions<'a> {
    pub status_description: &'a str,
    pub issuetype_description: &'a str,
    pub assignee_account_id: &'a str,
    pub assignee_display_name: &'a str,
    pub assignee_email: Option<&'a str>,
    pub reporter: Option<(&'a str, &'a str)>,
    pub comment_author_account_id: &'a str,
    pub comment_collection_self: Option<&'a str>,
    pub attachments: Option<serde_json::Value>,
}

fn build_comment_field(
    author_account_id: &str,
    collection_self: Option<&str>,
) -> serde_json::Value {
    let mut comment = serde_json::json!({
        "comments": [
            {
                "id": "100",
                "self": "https://example.atlassian.net/rest/api/3/issue/10001/comment/100",
                "author": {
                    "accountId": author_account_id,
                    "displayName": "Bob Dev",
                    "active": true,
                    "self": format!(
                        "https://example.atlassian.net/rest/api/3/user?accountId={author_account_id}"
                    ),
                    "avatarUrls": {}
                },
                "body": "Reproduced on v2.1.",
                "created": "2026-06-29T10:00:00.000+0000",
                "updated": "2026-06-29T10:00:00.000+0000"
            }
        ],
        "maxResults": 1,
        "total": 1,
        "startAt": 0
    });
    if let Some(self_url) = collection_self {
        comment["self"] = serde_json::json!(self_url);
    }
    comment
}

/// The Jira Cloud `GET /issue/{key}` response shape, parametrized on every
/// field `client.rs` and `commands.rs` set differently. See
/// `IssuePayloadOptions` for the union of those differences.
pub(crate) fn build_issue_payload(opts: IssuePayloadOptions) -> serde_json::Value {
    let mut assignee = serde_json::json!({
        "accountId": opts.assignee_account_id,
        "displayName": opts.assignee_display_name,
        "active": true,
        "self": format!(
            "https://example.atlassian.net/rest/api/3/user?accountId={}",
            opts.assignee_account_id
        ),
        "avatarUrls": {}
    });
    if let Some(email) = opts.assignee_email {
        assignee["emailAddress"] = serde_json::json!(email);
    }

    let mut fields = serde_json::json!({
        "summary": "Fix the login bug",
        "status": {
            "id": "3",
            "name": "In Progress",
            "description": opts.status_description,
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
            "description": opts.issuetype_description,
            "iconUrl": "https://example.atlassian.net/images/icons/issuetypes/bug.png",
            "self": "https://example.atlassian.net/rest/api/3/issuetype/10002",
            "subtask": false
        },
        "assignee": assignee,
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
                        { "type": "text", "text": "Login fails when MFA is enabled." }
                    ]
                }
            ]
        },
        "comment": build_comment_field(opts.comment_author_account_id, opts.comment_collection_self)
    });

    if let Some(attachments) = opts.attachments {
        fields["attachment"] = attachments;
    }
    if let Some((account_id, display_name)) = opts.reporter {
        fields["reporter"] = serde_json::json!({
            "accountId": account_id,
            "displayName": display_name,
            "active": true,
            "self": format!(
                "https://example.atlassian.net/rest/api/3/user?accountId={account_id}"
            ),
            "avatarUrls": {}
        });
    }

    serde_json::json!({
        "id": "10001",
        "key": "PROJ-123",
        "self": "https://example.atlassian.net/rest/api/3/issue/10001",
        "fields": fields
    })
}

pub(crate) fn build_myself_payload() -> serde_json::Value {
    serde_json::json!({
        "accountId": "5b10a2844c20165700ede21g",
        "displayName": "Alice Example",
        "emailAddress": "alice@example.com",
        "active": true,
        "self": "https://example.atlassian.net/rest/api/3/user?accountId=5b10a2844c20165700ede21g",
        "avatarUrls": {}
    })
}

// --- shared instance / search-payload builders (formerly duplicated between
// tests/unit/tui.rs and tests/unit/tui/shell.rs) ---

pub(crate) fn make_test_instance() -> crate::store::instances::Instance {
    crate::store::instances::Instance {
        name: "test".to_owned(),
        base_url: "https://test.atlassian.net".to_owned(),
        email: "test@example.com".to_owned(),
        token: "token".to_owned(),
        account_id: None,
    }
}

pub(crate) fn build_search_payload_with_key(key: &str) -> serde_json::Value {
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
