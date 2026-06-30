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
    /// Raw ADF document serialized as a JSON string, or plain text for older APIs.
    /// Callers must pass this through `adf_to_plain_text` before displaying.
    pub description: Option<String>,
    pub comments: Vec<IssueComment>,
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
}

/// The result of a JQL search — a page of issue rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub issues: Vec<IssueRow>,
    pub total: u64,
    pub is_last_page: bool,
}

#[cfg(test)]
#[path = "../tests/unit/models.rs"]
mod tests;
