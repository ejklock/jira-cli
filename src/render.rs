use crate::i18n::t;
use crate::models::{Issue, IssueComment, IssueRow};
use std::io::Write;

pub fn print_error(msg: &str) {
    eprintln!("{msg}");
}

/// Flatten an Atlassian Document Format (ADF) JSON string to plain text.
///
/// Handles paragraphs, text nodes, hardBreak, bulletList, orderedList, listItem,
/// heading, and codeBlock. Unknown node types fall back to their child text content.
/// If the input is not valid ADF JSON, returns the raw string as-is.
pub fn adf_to_plain_text(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };
    if value.get("type").and_then(|t| t.as_str()) != Some("doc") {
        return raw.to_string();
    }
    let mut out = String::new();
    for node in node_content(&value) {
        flatten_node(node, &mut out, 0);
    }
    out.trim_end_matches('\n').to_string()
}

fn node_content(node: &serde_json::Value) -> &[serde_json::Value] {
    node.get("content")
        .and_then(|c| c.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn flatten_node(node: &serde_json::Value, out: &mut String, list_depth: usize) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "paragraph" | "heading" => {
            flatten_inline_content(node, out);
            out.push('\n');
        }
        "codeBlock" => flatten_code_block(node, out),
        "bulletList" => flatten_list(node, out, list_depth, false),
        "orderedList" => flatten_list(node, out, list_depth, true),
        "blockquote" | "panel" => flatten_block_children(node, out, list_depth),
        "rule" => out.push_str("---\n"),
        _ => flatten_block_children(node, out, list_depth),
    }
}

fn flatten_code_block(node: &serde_json::Value, out: &mut String) {
    for child in node_content(node) {
        if let Some(text) = child.get("text").and_then(|t| t.as_str()) {
            out.push_str(text);
        }
    }
    out.push('\n');
}

fn flatten_list(node: &serde_json::Value, out: &mut String, depth: usize, ordered: bool) {
    for (i, item) in node_content(node).iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", i + 1)
        } else {
            "- ".to_string()
        };
        flatten_list_item(item, out, depth, &marker);
    }
}

fn flatten_block_children(node: &serde_json::Value, out: &mut String, list_depth: usize) {
    for child in node_content(node) {
        flatten_node(child, out, list_depth);
    }
}

fn flatten_list_item(item: &serde_json::Value, out: &mut String, depth: usize, marker: &str) {
    let indent = "  ".repeat(depth);
    for (i, child) in node_content(item).iter().enumerate() {
        flatten_list_item_child(child, out, depth, &indent, i == 0, marker);
    }
}

fn flatten_list_item_child(
    child: &serde_json::Value,
    out: &mut String,
    depth: usize,
    indent: &str,
    is_first: bool,
    marker: &str,
) {
    let child_type = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if child_type == "paragraph" {
        if is_first {
            out.push_str(&format!("{indent}{marker}"));
            flatten_inline_content(child, out);
            out.push('\n');
        } else {
            flatten_node(child, out, depth + 1);
        }
    } else if child_type == "bulletList" || child_type == "orderedList" {
        flatten_node(child, out, depth + 1);
    } else {
        flatten_node(child, out, depth);
    }
}

fn flatten_inline_content(node: &serde_json::Value, out: &mut String) {
    for child in node_content(node) {
        flatten_inline_node(child, out);
    }
}

fn push_attr(node: &serde_json::Value, key: &str, out: &mut String) {
    if let Some(text) = node
        .get("attrs")
        .and_then(|a| a.get(key))
        .and_then(|t| t.as_str())
    {
        out.push_str(text);
    }
}

fn flatten_inline_node(node: &serde_json::Value, out: &mut String) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "text" => {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                out.push_str(text);
            }
        }
        "hardBreak" => out.push('\n'),
        "mention" => push_attr(node, "text", out),
        "emoji" => push_attr(node, "shortName", out),
        "inlineCard" => push_attr(node, "url", out),
        _ => {
            for child in node_content(node) {
                flatten_inline_node(child, out);
            }
        }
    }
}

/// Render a list of issue rows as a human-readable table with columns:
/// KEY  TYPE  STATUS  ASSIGNEE  SUMMARY
pub fn render_issue_table(out: &mut dyn Write, rows: &[IssueRow]) {
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}",
        t("KEY"),
        t("TYPE"),
        t("STATUS"),
        t("ASSIGNEE"),
        t("SUMMARY")
    )
    .ok();
    for row in rows {
        let assignee = row
            .assignee
            .as_deref()
            .map(|s| s.to_owned())
            .unwrap_or_else(|| t("Unassigned"));
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            row.key, row.issue_type, row.status, assignee, row.summary
        )
        .ok();
    }
}

/// Build the canonical browse URL for a Jira issue.
///
/// Trims a trailing slash from `base_url` so the caller does not need to
/// normalise it — `"https://acme.atlassian.net/"` and
/// `"https://acme.atlassian.net"` both yield the same URL.
pub fn issue_browse_url(base_url: &str, key: &str) -> String {
    format!("{}/browse/{}", base_url.trim_end_matches('/'), key)
}

/// Render a Jira issue in human-readable form.
pub fn render_issue_human(
    issue: &Issue,
    instance_name: &str,
    base_url: &str,
    no_comments: bool,
    out: &mut dyn Write,
) {
    let issue_url = issue_browse_url(base_url, &issue.key);
    let description_text = issue
        .description
        .as_deref()
        .map(adf_to_plain_text)
        .unwrap_or_default();
    let assignee_name = issue
        .assignee
        .as_ref()
        .map(|a| a.display_name.clone())
        .unwrap_or_else(|| t("Unassigned"));
    let reporter_name = issue
        .reporter
        .as_ref()
        .map(|r| r.display_name.as_str())
        .unwrap_or("-");

    writeln!(out, "  [{instance_name}] {}", issue.key).ok();
    writeln!(out, "  {}", issue.summary).ok();
    writeln!(out, "  {}: {issue_url}", t("URL")).ok();
    writeln!(
        out,
        "  {}: {} ({})",
        t("Status"),
        issue.status,
        issue.status_category.as_deref().unwrap_or("-")
    )
    .ok();
    writeln!(out, "  {}: {}", t("Type"), issue.issue_type).ok();
    writeln!(
        out,
        "  {}: {}",
        t("Priority"),
        issue.priority.as_deref().unwrap_or("-")
    )
    .ok();
    writeln!(out, "  {}: {assignee_name}", t("Assignee")).ok();
    writeln!(out, "  {}: {reporter_name}", t("Reporter")).ok();
    if let Some(created) = &issue.created {
        writeln!(out, "  {}: {created}", t("Created")).ok();
    }
    if let Some(updated) = &issue.updated {
        writeln!(out, "  {}: {updated}", t("Updated")).ok();
    }
    if !description_text.is_empty() {
        writeln!(out, "\n{}:\n{description_text}", t("Description")).ok();
    }
    if !no_comments && !issue.comments.is_empty() {
        writeln!(out, "\n{}:", t("Comments")).ok();
        for comment in &issue.comments {
            render_comment_human(comment, out);
        }
    }
}

fn render_comment_human(comment: &IssueComment, out: &mut dyn Write) {
    let author = comment
        .author
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| t("Unknown"));
    let created = comment.created.as_deref().unwrap_or("");
    let body = adf_to_plain_text(&comment.body);
    writeln!(out, "  [{author}] {created}").ok();
    writeln!(out, "  {body}").ok();
    writeln!(out).ok();
}

#[cfg(test)]
pub struct LinkSegment {
    pub text: String,
    pub is_link: bool,
}

#[cfg(test)]
pub fn link_segments(line: &str) -> Vec<LinkSegment> {
    let mut result = Vec::new();
    let mut remaining = line;
    if remaining.is_empty() {
        result.push(LinkSegment {
            text: String::new(),
            is_link: false,
        });
        return result;
    }
    while !remaining.is_empty() {
        if let Some(start) = find_url_start_test(remaining) {
            if start > 0 {
                result.push(LinkSegment {
                    text: remaining[..start].to_string(),
                    is_link: false,
                });
            }
            let url_part = &remaining[start..];
            let end = url_part
                .find(|c: char| c.is_whitespace())
                .unwrap_or(url_part.len());
            result.push(LinkSegment {
                text: url_part[..end].to_string(),
                is_link: true,
            });
            remaining = &url_part[end..];
        } else {
            result.push(LinkSegment {
                text: remaining.to_string(),
                is_link: false,
            });
            break;
        }
    }
    result
}

#[cfg(test)]
fn find_url_start_test(s: &str) -> Option<usize> {
    for prefix in &["https://", "http://", "www."] {
        if let Some(pos) = s.find(prefix) {
            return Some(pos);
        }
    }
    None
}

#[cfg(test)]
#[path = "../tests/unit/render.rs"]
mod tests;
