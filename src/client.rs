#![allow(dead_code)]

use crate::models::{Issue, IssueAssignee, IssueComment, IssueRow, Myself, SearchResult};
use crate::store::instances::Instance;
use anyhow::{anyhow, Result};
use gouqi::core::SearchApiVersion;
use gouqi::{Credentials, SearchOptions};

/// The thin trait that commands depend on — never a gouqi type crosses this boundary.
#[async_trait::async_trait]
pub trait JiraClient: Send + Sync {
    async fn get_issue(&self, key: &str) -> Result<Issue>;
    async fn search(&self, jql: &str, max_results: u64) -> Result<SearchResult>;
    async fn search_page(
        &self,
        jql: &str,
        max_results: u64,
        page_token: &str,
    ) -> Result<SearchResult>;
    async fn myself(&self) -> Result<Myself>;
}

/// The single place where a `gouqi::async::Jira` is constructed.
/// All calls are pinned to the `instance.base_url` — no caller can override the host.
pub struct GouqiJiraClient {
    jira: gouqi::r#async::Jira,
}

impl GouqiJiraClient {
    /// Construct a client bound to the given instance.
    /// This is the only construction site for `gouqi::async::Jira` in the crate.
    pub fn new(instance: &Instance) -> Result<Self> {
        let credentials = Credentials::Basic(instance.email.clone(), instance.token.clone());
        let jira = gouqi::r#async::Jira::with_search_api_version(
            &instance.base_url,
            credentials,
            SearchApiVersion::V3,
        )
        .map_err(|e| anyhow!("Failed to build Jira client: {e}"))?;
        Ok(Self { jira })
    }
}

#[async_trait::async_trait]
impl JiraClient for GouqiJiraClient {
    async fn get_issue(&self, key: &str) -> Result<Issue> {
        // issues().get() uses /rest/api/latest; we must use v3 for Cloud ADF fields.
        let raw: gouqi::Issue = self
            .jira
            .get_versioned("api", Some("3"), &format!("/issue/{key}"))
            .await
            .map_err(|e| anyhow!("get_issue({key}): {e}"))?;
        map_gouqi_issue(raw)
    }

    async fn search(&self, jql: &str, max_results: u64) -> Result<SearchResult> {
        let capped = max_results.min(5000);
        let opts = SearchOptions::builder().max_results(capped).build();
        let raw = self
            .jira
            .search()
            .list(jql, &opts)
            .await
            .map_err(|e| anyhow!("search({jql}): {e}"))?;
        Ok(map_gouqi_search_results(raw))
    }

    async fn search_page(
        &self,
        jql: &str,
        max_results: u64,
        page_token: &str,
    ) -> Result<SearchResult> {
        let capped = max_results.min(5000);
        let opts = SearchOptions::builder()
            .max_results(capped)
            .next_page_token(page_token)
            .build();
        let raw = self
            .jira
            .search()
            .list(jql, &opts)
            .await
            .map_err(|e| anyhow!("search_page({jql}): {e}"))?;
        Ok(map_gouqi_search_results(raw))
    }

    async fn myself(&self) -> Result<Myself> {
        let raw: gouqi::User = self
            .jira
            .get_versioned("api", Some("3"), "/myself")
            .await
            .map_err(|e| anyhow!("myself(): {e}"))?;
        let account_id = raw
            .account_id
            .ok_or_else(|| anyhow!("myself() response missing accountId"))?;
        Ok(Myself {
            account_id,
            display_name: raw.display_name,
        })
    }
}

/// Map a gouqi `rep::SearchResults` page to our curated `SearchResult` domain type.
/// Shared by `search` and `search_page` — the only construction/mapping site for either.
fn map_gouqi_search_results(raw: gouqi::SearchResults) -> SearchResult {
    let issues = raw
        .issues
        .into_iter()
        .map(|i| IssueRow {
            key: i.key.clone(),
            issue_type: i.issue_type().map(|t| t.name).unwrap_or_default(),
            summary: i.summary().unwrap_or_default(),
            status: i.status().map(|s| s.name).unwrap_or_default(),
            assignee: i.assignee().map(|u| u.display_name),
        })
        .collect();

    SearchResult {
        issues,
        total: raw.total,
        is_last_page: raw.is_last_page.unwrap_or(true),
        next_page_token: raw.next_page_token,
    }
}

/// Map a gouqi `rep::Issue` to our curated `Issue` domain type.
/// gouqi types must not escape this function.
fn map_gouqi_issue(raw: gouqi::Issue) -> Result<Issue> {
    let summary = raw.summary().unwrap_or_default();
    let status_name = raw.status().map(|s| s.name).unwrap_or_default();
    let issue_type = raw
        .issue_type()
        .map(|t| t.name)
        .unwrap_or_else(|| "Unknown".to_string());
    let priority = raw.priority().map(|p| p.name);

    let assignee = raw.assignee().map(|u| IssueAssignee {
        display_name: u.display_name,
        account_id: u.account_id,
    });

    let reporter = raw.reporter().map(|u| IssueAssignee {
        display_name: u.display_name,
        account_id: u.account_id,
    });

    let created = raw.created().map(|dt| dt.to_string());
    let updated = raw.updated().map(|dt| dt.to_string());
    let duedate = extract_duedate(&raw);

    let description = raw.description();

    let status_category = extract_status_category(&raw);

    let comments = map_comments(&raw);

    Ok(Issue {
        key: raw.key,
        summary,
        status: status_name,
        status_category,
        issue_type,
        assignee,
        reporter,
        priority,
        created,
        updated,
        duedate,
        description,
        comments,
    })
}

/// Extract the raw `duedate` field (`"YYYY-MM-DD"` or absent) from the fields BTreeMap.
/// gouqi has no typed due-date accessor, so this reads the raw JSON directly, mirroring
/// `extract_status_category`.
fn extract_duedate(raw: &gouqi::Issue) -> Option<String> {
    raw.fields.get("duedate")?.as_str().map(str::to_string)
}

/// Extract `statusCategory.key` from the raw fields BTreeMap.
/// gouqi's `Status` struct does not capture `statusCategory`, so we read it directly.
/// The key holds the ADR 0004 literal category key (e.g. `new`, `indeterminate`, `done`),
/// not the display name.
fn extract_status_category(raw: &gouqi::Issue) -> Option<String> {
    let status_val = raw.fields.get("status")?;
    status_val
        .get("statusCategory")
        .and_then(|sc| sc.get("key"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
}

fn map_comments(raw: &gouqi::Issue) -> Vec<IssueComment> {
    raw.comments()
        .map(|c| {
            c.comments
                .into_iter()
                .map(|comment| IssueComment {
                    id: comment.id,
                    author: comment.author.map(|u| u.display_name),
                    body: comment.body.to_string(),
                    created: comment.created.map(|dt| dt.to_string()),
                    updated: comment.updated.map(|dt| dt.to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../tests/unit/client.rs"]
mod tests;
