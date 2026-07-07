#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// The curated assignee — display name and Cloud account ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueAssignee {
    pub display_name: String,
    pub account_id: Option<String>,
}

/// A single comment on a Jira issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: Option<String>,
    pub author: Option<String>,
    pub body: String,
    pub created: Option<String>,
    pub updated: Option<String>,
}

/// A single attachment on a Jira issue (ADR 0020 / BDR 0012).
/// Only the curated fields the tool uses are present — not a mirror of the
/// raw `fields.attachment` entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub url: String,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}

/// The curated domain model for a Jira Cloud issue.
/// Only the fields the tool actually uses are present here — not a mirror of gouqi's rep types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub status_category: Option<String>,
    pub issue_type: String,
    pub assignee: Option<IssueAssignee>,
    pub reporter: Option<IssueAssignee>,
    pub priority: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    /// Raw Jira due date as `"YYYY-MM-DD"`, or `None` when unset.
    /// `#[serde(default)]` keeps older cached JSON (written before this field
    /// existed) deserializable without a migration.
    #[serde(default)]
    pub duedate: Option<String>,
    /// Raw ADF document serialized as a JSON string, or plain text for older APIs.
    /// Callers must pass this through `adf_to_plain_text` before displaying.
    pub description: Option<String>,
    pub comments: Vec<IssueComment>,
    /// Curated attachment metadata (ADR 0020 / BDR 0012). `#[serde(default)]`
    /// keeps older cached issues (written before this field existed)
    /// deserializable without a migration, mirroring `duedate`.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// Curated representation of the authenticated user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Myself {
    pub account_id: String,
    pub display_name: String,
}

/// A summary row used in search result lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRow {
    pub key: String,
    pub issue_type: String,
    pub summary: String,
    pub status: String,
    pub assignee: Option<String>,
    /// Raw Jira due date as `"YYYY-MM-DD"`, or `None` when unset. TUI-only in
    /// this slice (ADR 0004 freezes the CLI table and agent_json list shape).
    /// `#[serde(default)]` keeps pre-field cached JSON deserializable.
    #[serde(default)]
    pub duedate: Option<String>,
    /// The issue's project name, falling back to the project key when the
    /// name is absent. TUI-only in this slice; `#[serde(default)]` keeps
    /// pre-field cached JSON deserializable.
    #[serde(default)]
    pub project: Option<String>,
}

/// The result of a JQL search — a page of issue rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub issues: Vec<IssueRow>,
    pub total: u64,
    pub is_last_page: bool,
    /// V3 pagination token for fetching the next page, `None` on the last page.
    pub next_page_token: Option<String>,
}

#[cfg(test)]
#[path = "../tests/unit/models.rs"]
mod tests;
