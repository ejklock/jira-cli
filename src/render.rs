use crate::i18n::{t, tf};
use crate::models::{Issue, IssueComment, IssueRow};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Neutral, ratatui-free style flags for one inline text run in a rich-rendered
/// ADF document. `link` retains the href for a later clickable-links slice (A2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RichStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub underline: bool,
    pub link: Option<String>,
}

/// One styled text run within a rich-rendered line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichSpan {
    pub text: String,
    pub style: RichStyle,
}

/// A single displayed line, made up of one or more styled runs.
pub type RichLine = Vec<RichSpan>;

/// Walk an ADF JSON string into styled lines, mirroring `adf_to_plain_text`'s
/// block shaping (paragraphs, lists, code blocks, hardBreak) but carrying each
/// inline mark (`strong`/`em`/`code`/`strike`/`underline`/`link`) into a
/// [`RichStyle`] per text run. Non-ADF / non-`doc` input yields a single
/// unstyled line with the raw string.
pub fn adf_to_rich(raw: &str) -> Vec<RichLine> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return vec![single_unstyled_line(raw)];
    };
    if value.get("type").and_then(|t| t.as_str()) != Some("doc") {
        return vec![single_unstyled_line(raw)];
    }
    let mut lines: Vec<RichLine> = Vec::new();
    let mut current: RichLine = Vec::new();
    for node in node_content(&value) {
        rich_node(node, &mut lines, &mut current, 0);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn single_unstyled_line(text: &str) -> RichLine {
    vec![RichSpan {
        text: text.to_string(),
        style: RichStyle::default(),
    }]
}

fn rich_node(
    node: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
    list_depth: usize,
) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "paragraph" | "heading" => {
            rich_inline_content(node, lines, current);
            lines.push(std::mem::take(current));
        }
        "codeBlock" => rich_code_block(node, lines, current),
        "bulletList" => rich_list(node, lines, current, list_depth, false),
        "orderedList" => rich_list(node, lines, current, list_depth, true),
        "blockquote" | "panel" => rich_block_children(node, lines, current, list_depth),
        "rule" => {
            current.push(RichSpan {
                text: "---".to_string(),
                style: RichStyle::default(),
            });
            lines.push(std::mem::take(current));
        }
        _ => rich_block_children(node, lines, current, list_depth),
    }
}

fn rich_code_block(node: &serde_json::Value, lines: &mut Vec<RichLine>, current: &mut RichLine) {
    let style = RichStyle {
        code: true,
        ..RichStyle::default()
    };
    for child in node_content(node) {
        if let Some(text) = child.get("text").and_then(|t| t.as_str()) {
            current.push(RichSpan {
                text: text.to_string(),
                style: style.clone(),
            });
        }
    }
    lines.push(std::mem::take(current));
}

fn rich_list(
    node: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
    depth: usize,
    ordered: bool,
) {
    for (i, item) in node_content(node).iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", i + 1)
        } else {
            "- ".to_string()
        };
        rich_list_item(item, lines, current, depth, &marker);
    }
}

fn rich_block_children(
    node: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
    list_depth: usize,
) {
    for child in node_content(node) {
        rich_node(child, lines, current, list_depth);
    }
}

fn rich_list_item(
    item: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
    depth: usize,
    marker: &str,
) {
    let indent = "  ".repeat(depth);
    for (i, child) in node_content(item).iter().enumerate() {
        rich_list_item_child(child, lines, current, depth, &indent, i == 0, marker);
    }
}

#[allow(clippy::too_many_arguments)]
fn rich_list_item_child(
    child: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
    depth: usize,
    indent: &str,
    is_first: bool,
    marker: &str,
) {
    let child_type = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if child_type == "paragraph" {
        if is_first {
            current.push(RichSpan {
                text: format!("{indent}{marker}"),
                style: RichStyle::default(),
            });
            rich_inline_content(child, lines, current);
            lines.push(std::mem::take(current));
        } else {
            rich_node(child, lines, current, depth + 1);
        }
    } else if child_type == "bulletList" || child_type == "orderedList" {
        rich_node(child, lines, current, depth + 1);
    } else {
        rich_node(child, lines, current, depth);
    }
}

fn rich_inline_content(
    node: &serde_json::Value,
    lines: &mut Vec<RichLine>,
    current: &mut RichLine,
) {
    for child in node_content(node) {
        rich_inline_node(child, lines, current);
    }
}

fn rich_inline_node(node: &serde_json::Value, lines: &mut Vec<RichLine>, current: &mut RichLine) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match node_type {
        "text" => {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                current.push(RichSpan {
                    text: text.to_string(),
                    style: marks_to_style(node),
                });
            }
        }
        "hardBreak" => lines.push(std::mem::take(current)),
        "mention" => push_attr_span(node, "text", current),
        "emoji" => push_attr_span(node, "shortName", current),
        "inlineCard" => push_attr_span(node, "url", current),
        _ => {
            for child in node_content(node) {
                rich_inline_node(child, lines, current);
            }
        }
    }
}

fn push_attr_span(node: &serde_json::Value, key: &str, current: &mut RichLine) {
    if let Some(text) = node
        .get("attrs")
        .and_then(|a| a.get(key))
        .and_then(|t| t.as_str())
    {
        current.push(RichSpan {
            text: text.to_string(),
            style: RichStyle::default(),
        });
    }
}

fn marks_to_style(node: &serde_json::Value) -> RichStyle {
    let mut style = RichStyle::default();
    let Some(marks) = node.get("marks").and_then(|m| m.as_array()) else {
        return style;
    };
    for mark in marks {
        apply_mark(mark, &mut style);
    }
    style
}

fn apply_mark(mark: &serde_json::Value, style: &mut RichStyle) {
    match mark.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "strong" => style.bold = true,
        "em" => style.italic = true,
        "code" => style.code = true,
        "strike" => style.strike = true,
        "underline" => style.underline = true,
        "link" => {
            style.underline = true;
            style.link = mark
                .get("attrs")
                .and_then(|a| a.get("href"))
                .and_then(|h| h.as_str())
                .map(str::to_string);
        }
        _ => {}
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

/// Convert a proleptic Gregorian civil date to a day count since the Unix epoch
/// (1970-01-01 == 0). Howard Hinnant's `days_from_civil` algorithm — pure integer
/// arithmetic, correct for the full `i64` range, no external date crate needed.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Parse a `"YYYY-MM-DD"` due date into a day count via [`days_from_civil`].
/// Any malformed input (wrong segment count or non-numeric part) yields `None`.
fn parse_due_days(duedate: &str) -> Option<i64> {
    let mut parts = duedate.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// Bucket a day-delta (due - today) into the localized relative-due string.
/// Future is always plural (`delta >= 2`); singular only for the fixed `-1` case.
fn bucket_relative_due(delta: i64) -> String {
    match delta {
        0 => t("today"),
        1 => t("tomorrow"),
        n if n >= 2 => tf("in {n} days", &[("n", &n.to_string())]),
        -1 => t("overdue by 1 day"),
        n => tf("overdue by {n} days", &[("n", &(-n).to_string())]),
    }
}

/// The day-delta (`due - today`) behind a Jira due date, or `None` when
/// `duedate` fails to parse. Extracted so callers (e.g. the list card
/// renderer) can map a delta to a display style without duplicating
/// `parse_due_days`'s date math.
pub fn due_day_delta(duedate: &str, today_days: i64) -> Option<i64> {
    let due_days = parse_due_days(duedate)?;
    Some(due_days - today_days)
}

/// Render a Jira due date as a localized relative string ("today" / "tomorrow" /
/// "in N days" / "overdue by N days"), or `None` when `duedate` fails to parse.
/// `today_days` is injected (never read from the clock here) so this stays pure
/// and table-testable; callers derive it from [`days_from_civil`] applied to the
/// current date.
pub(crate) fn relative_due(duedate: &str, today_days: i64) -> Option<String> {
    due_day_delta(duedate, today_days).map(bucket_relative_due)
}

/// The current UTC date as a `days_from_civil` day count, for `relative_due`'s
/// `today_days` argument. Impure (reads the clock) so `relative_due` itself
/// doesn't have to be.
pub(crate) fn today_days_now() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();
    let (year, month, day, _, _, _) = crate::store::secs_to_utc_parts(secs);
    days_from_civil(year as i64, month as i64, day as i64)
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
    let due_line = issue
        .duedate
        .as_deref()
        .and_then(|d| relative_due(d, today_days_now()));
    if let Some(due) = due_line {
        writeln!(out, "  {}: {due}", t("Due")).ok();
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
