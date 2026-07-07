use super::*;
use crate::models::{Issue, IssueAssignee, IssueComment, IssueRow};
use crate::test_support::attachment;

fn sample_issue() -> Issue {
    Issue {
        key: "PROJ-123".to_string(),
        summary: "Fix the login bug".to_string(),
        status: "In Progress".to_string(),
        status_category: Some("indeterminate".to_string()),
        issue_type: "Bug".to_string(),
        assignee: Some(IssueAssignee {
            display_name: "Alice Example".to_string(),
            account_id: Some("5b10a2844c20165700ede21g".to_string()),
        }),
        reporter: Some(IssueAssignee {
            display_name: "John Reporter".to_string(),
            account_id: Some("rep-account-id".to_string()),
        }),
        priority: Some("High".to_string()),
        created: Some("2026-01-02T10:00:00.000+0000".to_string()),
        updated: Some("2026-01-09T12:00:00.000+0000".to_string()),
        duedate: None,
        description: Some(r#"{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"plain text description"}]}]}"#.to_string()),
        comments: vec![IssueComment {
            id: Some("100".to_string()),
            author: Some("John".to_string()),
            body: "A comment body.".to_string(),
            created: Some("2026-01-03T14:22:00.000+0000".to_string()),
            updated: None,
        }],
        attachments: vec![],
    }
}

// --- issue_object: ref field ---

#[test]
fn issue_object_ref_equals_issue_key() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["ref"], "PROJ-123", "ref must be the issue key");
}

// --- issue_object: instance field ---

#[test]
fn issue_object_instance_equals_provided_name() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["instance"], "work");
}

// --- issue_object: project_key derived from key prefix ---

#[test]
fn issue_object_project_key_is_prefix_of_key() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["project_key"], "PROJ",
        "project_key must be 'PROJ' for PROJ-123"
    );
}

// --- issue_object: key field ---

#[test]
fn issue_object_key_equals_issue_key() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["key"], "PROJ-123");
}

// --- issue_object: summary ---

#[test]
fn issue_object_summary_equals_issue_summary() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["summary"], "Fix the login bug");
}

// --- issue_object: status literal ---

#[test]
fn issue_object_status_is_literal_status_name() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["status"], "In Progress");
}

// --- issue_object: status_category is the category KEY ---

#[test]
fn issue_object_status_category_is_category_key() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["status_category"], "indeterminate",
        "status_category must be the category KEY, not the display label"
    );
}

// --- issue_object: issue_type ---

#[test]
fn issue_object_issue_type_equals_issue_type() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["issue_type"], "Bug");
}

// --- issue_object: assignee resolved display name ---

#[test]
fn issue_object_assignee_is_display_name() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["assignee"], "Alice Example");
}

#[test]
fn issue_object_assignee_id_is_account_id() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["assignee_id"], "5b10a2844c20165700ede21g");
}

#[test]
fn issue_object_assignee_null_when_unassigned() {
    let mut issue = sample_issue();
    issue.assignee = None;
    let obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["assignee"],
        serde_json::Value::Null,
        "unassigned must be null"
    );
    assert_eq!(obj["assignee_id"], serde_json::Value::Null);
}

// --- issue_object: reporter ---

#[test]
fn issue_object_reporter_is_display_name() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["reporter"], "John Reporter");
}

// --- issue_object: url ---

#[test]
fn issue_object_url_is_browse_url() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(obj["url"], "https://acme.atlassian.net/browse/PROJ-123");
}

// --- issue_object: description is ADF-flattened plain text ---

#[test]
fn issue_object_description_is_adf_flattened() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["description"], "plain text description",
        "description must be ADF-flattened plain text"
    );
}

// --- issue_object: comments array ---

#[test]
fn issue_object_comments_included_when_no_comments_false() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    let comments = obj["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 1, "must include 1 comment");
    assert_eq!(comments[0]["author"], "John");
    assert_eq!(comments[0]["body"], "A comment body.");
}

#[test]
fn issue_object_no_comments_flag_yields_empty_array() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", true);
    let comments = obj["comments"].as_array().unwrap();
    assert_eq!(
        comments.len(),
        0,
        "no_comments=true must produce empty comments array"
    );
}

#[test]
fn issue_object_empty_comments_list_yields_empty_array() {
    let mut issue = sample_issue();
    issue.comments = vec![];
    let obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    let comments = obj["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 0, "empty comments must produce empty array");
}

// --- issue_object: attachments (ADR 0020 / BDR 0012 S2) ---

#[test]
fn issue_object_attachments_contains_both_entries_with_exact_shape() {
    let issue = Issue {
        attachments: vec![
            attachment(
                "screenshot.png",
                "https://acme.atlassian.net/attachments/1",
                Some("image/png"),
                Some(2048),
            ),
            attachment(
                "notes.txt",
                "https://acme.atlassian.net/attachments/2",
                None,
                None,
            ),
        ],
        ..sample_issue()
    };
    let obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    let attachments = obj["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 2, "both attachments must be present");

    let first = &attachments[0];
    let mut keys: Vec<&str> = first
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["filename", "mime_type", "size", "url"],
        "each attachment must have exactly {{filename, url, mime_type, size}}"
    );
    assert_eq!(first["filename"], "screenshot.png");
    assert_eq!(first["url"], "https://acme.atlassian.net/attachments/1");
    assert_eq!(first["mime_type"], "image/png");
    assert_eq!(first["size"], 2048);

    let second = &attachments[1];
    assert_eq!(second["filename"], "notes.txt");
    assert_eq!(
        second["mime_type"],
        serde_json::Value::Null,
        "absent mime_type must map to null"
    );
    assert_eq!(
        second["size"],
        serde_json::Value::Null,
        "absent size must map to null"
    );
}

#[test]
fn issue_object_no_attachments_yields_empty_array() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    let attachments = obj["attachments"].as_array().unwrap();
    assert_eq!(
        attachments.len(),
        0,
        "an issue with no attachments must yield an empty array"
    );
}

#[test]
fn issue_object_attachments_are_additive_pre_existing_keys_unchanged() {
    let issue = Issue {
        attachments: vec![attachment(
            "file.pdf",
            "https://acme.atlassian.net/attachments/1",
            Some("application/pdf"),
            Some(10),
        )],
        ..sample_issue()
    };
    let obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    assert!(
        obj["attachments"].as_array().unwrap().len() == 1,
        "attachments must be present alongside pre-existing keys"
    );
    assert_eq!(
        obj["comments"].as_array().unwrap().len(),
        1,
        "adding attachments must not change the pre-existing comments key"
    );
    assert_eq!(
        obj["duedate"],
        serde_json::Value::Null,
        "adding attachments must not change the pre-existing duedate key"
    );
}

// --- issue_object: raw duedate (issue 0026 A3b / ADR 0013) ---

#[test]
fn issue_object_duedate_is_raw_yyyy_mm_dd_string() {
    let issue = Issue {
        duedate: Some("2026-07-15".to_string()),
        ..sample_issue()
    };
    let obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["duedate"], "2026-07-15",
        "duedate must be the raw YYYY-MM-DD string, not a localized relative string"
    );
}

#[test]
fn issue_object_duedate_is_null_when_none() {
    let obj = issue_object(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert_eq!(
        obj["duedate"],
        serde_json::Value::Null,
        "duedate must be null when the issue has no due date"
    );
}

// --- issue_to_minified_json: one line, minified ---

#[test]
fn issue_to_minified_json_is_single_line() {
    let line = issue_to_minified_json(&sample_issue(), "work", "https://acme.atlassian.net", false);
    assert!(
        !line.contains('\n'),
        "minified output must be a single line: {line:?}"
    );
}

#[test]
fn issue_to_minified_json_is_valid_json() {
    let line = issue_to_minified_json(&sample_issue(), "work", "https://acme.atlassian.net", false);
    let _: serde_json::Value = serde_json::from_str(&line).expect("must be valid JSON");
}

#[test]
fn issue_to_minified_json_ref_equals_key() {
    let line = issue_to_minified_json(&sample_issue(), "work", "https://acme.atlassian.net", false);
    let obj: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(obj["ref"], "PROJ-123", "minified JSON ref must equal key");
}

#[test]
fn issue_to_minified_json_no_comments_flag_yields_empty_comments() {
    let line = issue_to_minified_json(&sample_issue(), "work", "https://acme.atlassian.net", true);
    let obj: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        obj["comments"].as_array().unwrap().len(),
        0,
        "no_comments must yield empty comments in minified JSON"
    );
}

// --- mine_list_object: shape lock (BDR 0005 Scn 2) ---

fn sample_rows() -> Vec<IssueRow> {
    vec![
        IssueRow {
            key: "PROJ-1".to_string(),
            issue_type: "Bug".to_string(),
            summary: "Fix the crash".to_string(),
            status: "Open".to_string(),
            assignee: Some("Alice".to_string()),
            duedate: None,
            project: None,
        },
        IssueRow {
            key: "PROJ-2".to_string(),
            issue_type: "Task".to_string(),
            summary: "Refactor module".to_string(),
            status: "In Progress".to_string(),
            assignee: None,
            duedate: None,
            project: None,
        },
    ]
}

#[test]
fn mine_list_object_top_level_keys_are_exactly_count_jql_issues() {
    let jql = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";
    let obj = mine_list_object(jql, &sample_rows());
    let map = obj.as_object().expect("must be a JSON object");
    let mut keys: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["count", "issues", "jql"],
        "top-level keys must be exactly {{count, jql, issues}}"
    );
}

#[test]
fn mine_list_object_count_equals_rows_len() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &sample_rows());
    assert_eq!(
        obj["count"].as_u64().unwrap(),
        2,
        "count must equal rows.len()"
    );
}

#[test]
fn mine_list_object_jql_equals_passed_string() {
    let jql = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";
    let obj = mine_list_object(jql, &sample_rows());
    assert_eq!(
        obj["jql"], jql,
        "jql field must equal the passed JQL string"
    );
}

#[test]
fn mine_list_object_each_issue_has_key_type_status_assignee_summary() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &sample_rows());
    let issues = obj["issues"].as_array().expect("issues must be an array");
    assert_eq!(issues.len(), 2, "must have 2 issues");
    for issue in issues {
        let map = issue.as_object().expect("each issue must be a JSON object");
        let mut keys: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["assignee", "key", "status", "summary", "type"],
            "each issue must have exactly {{key, type, status, assignee, summary}}"
        );
    }
}

#[test]
fn mine_list_object_assignee_is_display_name_when_present() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &sample_rows());
    let issues = obj["issues"].as_array().unwrap();
    assert_eq!(
        issues[0]["assignee"], "Alice",
        "assigned issue must have display name"
    );
}

#[test]
fn mine_list_object_assignee_is_null_when_none() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &sample_rows());
    let issues = obj["issues"].as_array().unwrap();
    assert_eq!(
        issues[1]["assignee"],
        serde_json::Value::Null,
        "unassigned must be JSON null"
    );
}

#[test]
fn mine_list_object_issue_fields_match_row_values() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &sample_rows());
    let issues = obj["issues"].as_array().unwrap();
    assert_eq!(issues[0]["key"], "PROJ-1");
    assert_eq!(issues[0]["type"], "Bug");
    assert_eq!(issues[0]["status"], "Open");
    assert_eq!(issues[0]["summary"], "Fix the crash");
}

#[test]
fn mine_list_object_empty_rows_yields_count_0_and_empty_issues() {
    let jql = "test jql";
    let obj = mine_list_object(jql, &[]);
    assert_eq!(
        obj["count"].as_u64().unwrap(),
        0,
        "empty rows must yield count 0"
    );
    let issues = obj["issues"].as_array().expect("issues must be an array");
    assert_eq!(
        issues.len(),
        0,
        "empty rows must yield an empty issues array"
    );
}

#[test]
fn mine_list_to_minified_json_is_single_line() {
    let jql = "test jql";
    let line = mine_list_to_minified_json(jql, &sample_rows());
    assert!(
        !line.contains('\n'),
        "minified output must be a single line: {line:?}"
    );
}

#[test]
fn mine_list_to_minified_json_is_valid_json() {
    let jql = "test jql";
    let line = mine_list_to_minified_json(jql, &sample_rows());
    let _: serde_json::Value = serde_json::from_str(&line).expect("must be valid JSON");
}

#[test]
fn mine_list_to_minified_json_empty_is_single_line_with_count_0() {
    let jql = "test jql";
    let line = mine_list_to_minified_json(jql, &[]);
    assert!(
        !line.contains('\n'),
        "empty minified output must be a single line: {line:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(&line).expect("must be valid JSON");
    assert_eq!(obj["count"].as_u64().unwrap(), 0, "empty must have count 0");
    let issues = obj["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 0, "empty must have empty issues array");
}

// --- shared helper invariant: agent_json and render use same ADF flatten ---

#[test]
fn agent_json_and_render_produce_same_description_text() {
    let issue = sample_issue();
    let json_obj = issue_object(&issue, "work", "https://acme.atlassian.net", false);
    let json_desc = json_obj["description"].as_str().unwrap();

    let mut render_out = Vec::new();
    crate::render::render_issue_human(
        &issue,
        "work",
        "https://acme.atlassian.net",
        false,
        &mut render_out,
    );
    let render_text = std::str::from_utf8(&render_out).unwrap();

    assert!(
        render_text.contains(json_desc),
        "human render must contain the same description text as agent_json.\njson_desc: {json_desc:?}\nrender_text: {render_text}"
    );
}
