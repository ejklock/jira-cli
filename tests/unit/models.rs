use super::*;
use crate::test_support::{comment, issue as issue_fixture};
use serde_json::json;

#[test]
fn issue_roundtrips_through_serde() {
    // Overrides `duedate`/`comments` explicitly: the shared fixture defaults them
    // to `None`/empty, but a serde round-trip must also exercise `Some(duedate)`
    // and a non-empty `comments` vec (itself with partial `None` fields).
    let issue = Issue {
        duedate: Some("2026-01-05".to_string()),
        comments: vec![comment(Some("10"), Some("Bob"), "Nice work", None, None)],
        ..issue_fixture("PROJ-1")
    };

    let serialized = serde_json::to_value(&issue).unwrap();
    let deserialized: Issue = serde_json::from_value(serialized).unwrap();
    assert_eq!(issue, deserialized);
}

#[test]
fn issue_null_assignee_deserializes_to_none() {
    let raw = json!({
        "key": "PROJ-2",
        "summary": "Task",
        "status": "Open",
        "status_category": null,
        "issue_type": "Bug",
        "assignee": null,
        "reporter": null,
        "priority": null,
        "created": null,
        "updated": null,
        "description": null,
        "comments": []
    });
    let issue: Issue = serde_json::from_value(raw).unwrap();
    assert_eq!(issue.assignee, None);
    assert_eq!(issue.priority, None);
    assert_eq!(issue.description, None);
    assert!(issue.comments.is_empty());
}

#[test]
fn myself_roundtrips_through_serde() {
    let myself = Myself {
        account_id: "abc123".to_string(),
        display_name: "Alice Example".to_string(),
    };
    let serialized = serde_json::to_value(&myself).unwrap();
    let deserialized: Myself = serde_json::from_value(serialized).unwrap();
    assert_eq!(myself, deserialized);
}

#[test]
fn search_result_with_multiple_rows() {
    let result = SearchResult {
        issues: vec![
            IssueRow {
                key: "A-1".to_string(),
                issue_type: "Story".to_string(),
                summary: "First".to_string(),
                status: "Open".to_string(),
                assignee: Some("Alice".to_string()),
                duedate: None,
                project: None,
            },
            IssueRow {
                key: "A-2".to_string(),
                issue_type: "Bug".to_string(),
                summary: "Second".to_string(),
                status: "Done".to_string(),
                assignee: None,
                duedate: None,
                project: None,
            },
        ],
        total: 2,
        is_last_page: true,
        next_page_token: None,
    };
    let serialized = serde_json::to_value(&result).unwrap();
    let deserialized: SearchResult = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized.issues.len(), 2);
    assert_eq!(deserialized.issues[0].key, "A-1");
    assert_eq!(deserialized.issues[1].assignee, None);
    assert!(deserialized.is_last_page);
    assert_eq!(deserialized.next_page_token, None);
}

#[test]
fn search_result_with_next_page_token_roundtrips_through_serde() {
    let result = SearchResult {
        issues: vec![],
        total: 100,
        is_last_page: false,
        next_page_token: Some("TOK2".to_string()),
    };
    let serialized = serde_json::to_value(&result).unwrap();
    let deserialized: SearchResult = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized.next_page_token.as_deref(), Some("TOK2"));
    assert!(!deserialized.is_last_page);
}

#[test]
fn issue_row_with_issue_type_roundtrips_through_serde() {
    let row = IssueRow {
        key: "PROJ-99".to_string(),
        issue_type: "Epic".to_string(),
        summary: "Big initiative".to_string(),
        status: "In Progress".to_string(),
        assignee: Some("Dev Alice".to_string()),
        duedate: None,
        project: None,
    };
    let serialized = serde_json::to_value(&row).unwrap();
    assert_eq!(
        serialized["issue_type"], "Epic",
        "issue_type must serialize"
    );
    let deserialized: IssueRow = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized, row);
    assert_eq!(deserialized.issue_type, "Epic");
}

#[test]
fn issue_row_unassigned_roundtrips_through_serde() {
    let row = IssueRow {
        key: "PROJ-100".to_string(),
        issue_type: "Task".to_string(),
        summary: "Unassigned task".to_string(),
        status: "Open".to_string(),
        assignee: None,
        duedate: None,
        project: None,
    };
    let serialized = serde_json::to_value(&row).unwrap();
    let deserialized: IssueRow = serde_json::from_value(serialized).unwrap();
    assert_eq!(deserialized.assignee, None);
    assert_eq!(deserialized.issue_type, "Task");
}

#[test]
fn issue_comment_without_author_deserializes_safely() {
    let raw = json!({
        "id": null,
        "author": null,
        "body": "Anonymous comment",
        "created": null,
        "updated": null
    });
    let comment: IssueComment = serde_json::from_value(raw).unwrap();
    assert_eq!(comment.id, None);
    assert_eq!(comment.author, None);
    assert_eq!(comment.body, "Anonymous comment");
}

#[test]
fn issue_assignee_with_null_account_id() {
    let raw = json!({
        "display_name": "Server User",
        "account_id": null
    });
    let assignee: IssueAssignee = serde_json::from_value(raw).unwrap();
    assert_eq!(assignee.display_name, "Server User");
    assert_eq!(assignee.account_id, None);
}
