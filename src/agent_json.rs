use crate::models::{Issue, IssueComment, IssueRow};
use crate::render::adf_to_plain_text;
use crate::store::cache::derive_project_key;
use serde_json::{json, Value};

/// Build the curated agent_json object for a single Jira issue (ADR 0004).
///
/// The `ref` field is the bare issue key — the exact form `get` accepts.
/// ADF description is flattened via the same helper as the human renderer.
pub fn issue_object(
    issue: &Issue,
    instance_name: &str,
    base_url: &str,
    no_comments: bool,
) -> Value {
    let project_key = derive_project_key(&issue.key);
    let issue_url = crate::render::issue_browse_url(base_url, &issue.key);
    let description_text = issue
        .description
        .as_deref()
        .map(adf_to_plain_text)
        .unwrap_or_default();

    let assignee_name = issue
        .assignee
        .as_ref()
        .map(|a| Value::String(a.display_name.clone()))
        .unwrap_or(Value::Null);
    let assignee_id = issue
        .assignee
        .as_ref()
        .and_then(|a| a.account_id.as_ref())
        .map(|id| Value::String(id.clone()))
        .unwrap_or(Value::Null);
    let reporter_name = issue
        .reporter
        .as_ref()
        .map(|r| Value::String(r.display_name.clone()))
        .unwrap_or(Value::Null);
    let reporter_id = issue
        .reporter
        .as_ref()
        .and_then(|r| r.account_id.as_ref())
        .map(|id| Value::String(id.clone()))
        .unwrap_or(Value::Null);

    let comments = if no_comments {
        Value::Array(vec![])
    } else {
        Value::Array(issue.comments.iter().map(shape_comment).collect())
    };

    json!({
        "ref": issue.key,
        "instance": instance_name,
        "project_key": project_key,
        "key": issue.key,
        "summary": issue.summary,
        "status": issue.status,
        "status_category": issue.status_category,
        "issue_type": issue.issue_type,
        "assignee": assignee_name,
        "assignee_id": assignee_id,
        "reporter": reporter_name,
        "reporter_id": reporter_id,
        "priority": issue.priority,
        "created": issue.created,
        "updated": issue.updated,
        "url": issue_url,
        "description": description_text,
        "comments": comments,
    })
}

/// Serialise the issue object to a compact (minified) single-line JSON string.
pub fn issue_to_minified_json(
    issue: &Issue,
    instance_name: &str,
    base_url: &str,
    no_comments: bool,
) -> String {
    let obj = issue_object(issue, instance_name, base_url, no_comments);
    serde_json::to_string(&obj).expect("issue_object is always serialisable")
}

/// Build the curated list object for `mine --json` (BDR 0005 Scn 2, ADR 0004).
///
/// Serialises from IssueRow — the same struct `render_issue_table` uses — so the
/// table and the JSON list can never drift apart (NO DRIFT constraint).
pub fn mine_list_object(jql: &str, rows: &[IssueRow]) -> Value {
    let issues: Vec<Value> = rows
        .iter()
        .map(|r| {
            let assignee = r
                .assignee
                .as_deref()
                .map(|n| Value::String(n.to_owned()))
                .unwrap_or(Value::Null);
            json!({
                "key": r.key,
                "type": r.issue_type,
                "status": r.status,
                "assignee": assignee,
                "summary": r.summary,
            })
        })
        .collect();

    json!({
        "count": rows.len(),
        "jql": jql,
        "issues": issues,
    })
}

/// Serialise the mine list object to a compact (minified) single-line JSON string.
pub fn mine_list_to_minified_json(jql: &str, rows: &[IssueRow]) -> String {
    let obj = mine_list_object(jql, rows);
    serde_json::to_string(&obj).expect("mine_list_object is always serialisable")
}

fn shape_comment(comment: &IssueComment) -> Value {
    let body = adf_to_plain_text(&comment.body);
    json!({
        "author": comment.author,
        "author_id": Value::Null,
        "created": comment.created,
        "body": body,
    })
}

#[cfg(test)]
#[path = "../tests/unit/agent_json.rs"]
mod tests;
