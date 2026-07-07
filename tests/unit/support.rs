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
