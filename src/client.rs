#![allow(dead_code)]

use crate::models::{
    Attachment, CommentWriteResult, Issue, IssueAssignee, IssueComment, IssueRow, Myself,
    ProjectRow, SearchResult,
};
use crate::store::instances::Instance;
use anyhow::{anyhow, Result};
use gouqi::core::SearchApiVersion;
use gouqi::{Credentials, SearchOptions};
use std::fmt;

/// The client's typed error surface — never a raw `gouqi::Error` crosses this
/// boundary. `Unauthorized` is the single case callers may match on by type
/// (an HTTP 401); everything else keeps its previous `anyhow!`-wrapped text
/// so non-401 rendering stays byte-identical to before this variant existed.
#[derive(Debug)]
pub enum ClientError {
    Unauthorized { instance: String },
    Other(anyhow::Error),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Unauthorized { instance } => {
                write!(f, "Unauthorized for instance '{instance}'")
            }
            ClientError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}

pub type ClientResult<T> = std::result::Result<T, ClientError>;

/// The thin trait that commands depend on — never a gouqi type crosses this boundary.
#[async_trait::async_trait]
pub trait JiraClient: Send + Sync {
    async fn get_issue(&self, key: &str) -> ClientResult<Issue>;
    async fn search(&self, jql: &str, max_results: u64) -> ClientResult<SearchResult>;
    async fn search_page(
        &self,
        jql: &str,
        max_results: u64,
        page_token: &str,
    ) -> ClientResult<SearchResult>;
    async fn myself(&self) -> ClientResult<Myself>;
    async fn list_projects(&self) -> ClientResult<Vec<ProjectRow>>;
    async fn add_comment(&self, key: &str, body_text: &str) -> ClientResult<CommentWriteResult>;
    async fn update_comment(
        &self,
        key: &str,
        comment_id: &str,
        body_text: &str,
    ) -> ClientResult<CommentWriteResult>;
    async fn delete_comment(&self, key: &str, comment_id: &str) -> ClientResult<()>;
    /// Posts a brand-new top-level comment on `key` whose ADF carries a
    /// leading mention of `mention_account_id`/`mention_display` (ADR 0026
    /// §5, BDR 0017 S8): Jira comments are flat, so a "reply" is a new
    /// comment whose first content node is a real mention (notifies the
    /// mentioned account), followed by `body_text`.
    async fn reply_comment(
        &self,
        key: &str,
        mention_account_id: &str,
        mention_display: &str,
        body_text: &str,
    ) -> ClientResult<CommentWriteResult>;
}

/// Builds the paragraph `content` array shared by [`plain_text_to_adf`] and
/// [`mention_adf`]: the text split on `\n` and interleaved with `hardBreak`
/// nodes. Pure — no I/O. Empty text yields an empty array (ADR rejects empty
/// text nodes, so blank segments are never emitted).
fn plain_text_content(text: &str) -> Vec<serde_json::Value> {
    let mut lines = text.split('\n');
    let mut content = Vec::new();
    if let Some(first) = lines.next() {
        push_text_node(&mut content, first);
    }
    for line in lines {
        content.push(serde_json::json!({"type": "hardBreak"}));
        push_text_node(&mut content, line);
    }
    content
}

/// Build the minimal ADF document Jira Cloud's comment-write endpoints
/// require: a single paragraph whose text is split on `\n` and interleaved
/// with `hardBreak` nodes. Pure — no I/O. Empty text yields a structurally
/// valid doc with an empty paragraph `content` array (ADF rejects empty text
/// nodes, so blank segments are never emitted).
fn plain_text_to_adf(text: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": [{"type": "paragraph", "content": plain_text_content(text)}],
    })
}

/// Build the reply ADF document (ADR 0026 §5, BDR 0017 S8): a single
/// paragraph whose first content node is a real Jira `mention` node —
/// `attrs.id` is the mentioned account's id, `attrs.text` its `@display`
/// label — followed by a literal space and `body_text`'s own paragraph
/// content (via [`plain_text_content`], mirroring [`plain_text_to_adf`]'s
/// line-splitting). Pure — no I/O.
fn mention_adf(
    mention_account_id: &str,
    mention_display: &str,
    body_text: &str,
) -> serde_json::Value {
    let mut content = vec![
        serde_json::json!({
            "type": "mention",
            "attrs": {"id": mention_account_id, "text": format!("@{mention_display}")},
        }),
        serde_json::json!({"type": "text", "text": " "}),
    ];
    content.extend(plain_text_content(body_text));
    serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": [{"type": "paragraph", "content": content}],
    })
}

fn push_text_node(content: &mut Vec<serde_json::Value>, segment: &str) {
    if !segment.is_empty() {
        content.push(serde_json::json!({"type": "text", "text": segment}));
    }
}

/// gouqi 0.20 exposes no `put_versioned`/`delete_versioned` — only GET/POST
/// have versioned helpers. Its unversioned `put`/`delete` build
/// `rest/{api}/latest{endpoint}`; prefixing the endpoint with this
/// dot-segment makes the `url` crate's RFC 3986 parse-time normalization
/// collapse `latest/../3` into `3`, reaching the v3 (ADF) API without a
/// second HTTP client or construction site (ADR 0022 addendum). The
/// wiremock tests assert the literal received path is the falsifiability
/// guard for this workaround.
fn v3_write_endpoint(path: &str) -> String {
    format!("/../3{path}")
}

/// The server-assigned ID returned by the comment write endpoints — the only
/// shape parsed out of the raw POST/PUT response body before it is mapped
/// into the curated `CommentWriteResult`.
#[derive(serde::Deserialize)]
struct CommentIdResponse {
    id: String,
}

/// The single place where a `gouqi::async::Jira` is constructed.
/// All calls are pinned to the `instance.base_url` — no caller can override the host.
pub struct GouqiJiraClient {
    jira: gouqi::r#async::Jira,
    instance_name: String,
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
        Ok(Self {
            jira,
            instance_name: instance.name.clone(),
        })
    }

    /// Classify a raw `gouqi::Error` at the single wrapper boundary: HTTP 401
    /// (gouqi's own `Error::Unauthorized`, matched by type) becomes the typed
    /// `ClientError::Unauthorized` carrying this client's instance name; any
    /// other error keeps flowing through `wrap`'s existing `anyhow!` format,
    /// unchanged from before this mapping existed.
    fn classify_error(
        &self,
        e: gouqi::Error,
        wrap: impl FnOnce(gouqi::Error) -> anyhow::Error,
    ) -> ClientError {
        match e {
            gouqi::Error::Unauthorized => ClientError::Unauthorized {
                instance: self.instance_name.clone(),
            },
            other => ClientError::Other(wrap(other)),
        }
    }
}

#[async_trait::async_trait]
impl JiraClient for GouqiJiraClient {
    async fn get_issue(&self, key: &str) -> ClientResult<Issue> {
        // issues().get() uses /rest/api/latest; we must use v3 for Cloud ADF fields.
        let raw: gouqi::Issue = self
            .jira
            .get_versioned("api", Some("3"), &format!("/issue/{key}"))
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("get_issue({key}): {e}")))?;
        map_gouqi_issue(raw).map_err(ClientError::Other)
    }

    async fn search(&self, jql: &str, max_results: u64) -> ClientResult<SearchResult> {
        let capped = max_results.min(5000);
        let opts = SearchOptions::builder().max_results(capped).build();
        let raw = self
            .jira
            .search()
            .list(jql, &opts)
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("search({jql}): {e}")))?;
        Ok(map_gouqi_search_results(raw))
    }

    async fn search_page(
        &self,
        jql: &str,
        max_results: u64,
        page_token: &str,
    ) -> ClientResult<SearchResult> {
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
            .map_err(|e| self.classify_error(e, |e| anyhow!("search_page({jql}): {e}")))?;
        Ok(map_gouqi_search_results(raw))
    }

    async fn myself(&self) -> ClientResult<Myself> {
        let raw: gouqi::User = self
            .jira
            .get_versioned("api", Some("3"), "/myself")
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("myself(): {e}")))?;
        let account_id = raw
            .account_id
            .ok_or_else(|| anyhow!("myself() response missing accountId"))
            .map_err(ClientError::Other)?;
        Ok(Myself {
            account_id,
            display_name: raw.display_name,
        })
    }

    async fn list_projects(&self) -> ClientResult<Vec<ProjectRow>> {
        let raw: serde_json::Value = self
            .jira
            .get_versioned("api", Some("3"), "/project/search?maxResults=100")
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("list_projects(): {e}")))?;
        Ok(extract_project_rows(&raw))
    }

    async fn add_comment(&self, key: &str, body_text: &str) -> ClientResult<CommentWriteResult> {
        let body = serde_json::json!({ "body": plain_text_to_adf(body_text) });
        let raw: CommentIdResponse = self
            .jira
            .post_versioned("api", Some("3"), &format!("/issue/{key}/comment"), body)
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("add_comment({key}): {e}")))?;
        Ok(CommentWriteResult { id: raw.id })
    }

    async fn update_comment(
        &self,
        key: &str,
        comment_id: &str,
        body_text: &str,
    ) -> ClientResult<CommentWriteResult> {
        let body = serde_json::json!({ "body": plain_text_to_adf(body_text) });
        let endpoint = v3_write_endpoint(&format!("/issue/{key}/comment/{comment_id}"));
        let raw: CommentIdResponse = self.jira.put("api", &endpoint, body).await.map_err(|e| {
            self.classify_error(e, |e| anyhow!("update_comment({key}, {comment_id}): {e}"))
        })?;
        Ok(CommentWriteResult { id: raw.id })
    }

    async fn delete_comment(&self, key: &str, comment_id: &str) -> ClientResult<()> {
        let endpoint = v3_write_endpoint(&format!("/issue/{key}/comment/{comment_id}"));
        let result: Result<(), gouqi::Error> = self.jira.delete("api", &endpoint).await;
        result.map_err(|e| {
            self.classify_error(e, |e| anyhow!("delete_comment({key}, {comment_id}): {e}"))
        })
    }

    async fn reply_comment(
        &self,
        key: &str,
        mention_account_id: &str,
        mention_display: &str,
        body_text: &str,
    ) -> ClientResult<CommentWriteResult> {
        let body = serde_json::json!({
            "body": mention_adf(mention_account_id, mention_display, body_text)
        });
        let raw: CommentIdResponse = self
            .jira
            .post_versioned("api", Some("3"), &format!("/issue/{key}/comment"), body)
            .await
            .map_err(|e| self.classify_error(e, |e| anyhow!("reply_comment({key}): {e}")))?;
        Ok(CommentWriteResult { id: raw.id })
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
            duedate: extract_duedate(&i),
            project: extract_project(&i),
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
    let attachments = extract_attachments(&raw);

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
        attachments,
    })
}

/// Extract the raw `duedate` field (`"YYYY-MM-DD"` or absent) from the fields BTreeMap.
/// gouqi has no typed due-date accessor, so this reads the raw JSON directly, mirroring
/// `extract_status_category`.
fn extract_duedate(raw: &gouqi::Issue) -> Option<String> {
    raw.fields.get("duedate")?.as_str().map(str::to_string)
}

/// Extract the search result row's project display name, falling back to the
/// project key when the name is absent, from the raw fields BTreeMap.
/// Mirrors `extract_duedate`'s raw-field-access-with-graceful-`None` pattern —
/// `IssueRow` is TUI-only in this slice so this reads straight from JSON
/// rather than adding a typed accessor.
fn extract_project(raw: &gouqi::Issue) -> Option<String> {
    let project_val = raw.fields.get("project")?;
    project_val
        .get("name")
        .and_then(|n| n.as_str())
        .or_else(|| project_val.get("key").and_then(|k| k.as_str()))
        .map(str::to_string)
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

/// Extract `fields.attachment` from the raw fields BTreeMap into curated
/// `Attachment`s, mirroring `extract_duedate`'s raw-field-access pattern.
/// Absent, `null`, or non-array yields an empty vec; each array entry is
/// parsed by `parse_attachment_entry`, which skips (never errors on) an
/// entry missing its required `filename`/`content`.
fn extract_attachments(raw: &gouqi::Issue) -> Vec<Attachment> {
    raw.fields
        .get("attachment")
        .and_then(|v| v.as_array())
        .map(|entries| entries.iter().filter_map(parse_attachment_entry).collect())
        .unwrap_or_default()
}

/// Parse a single raw `fields.attachment` entry into a curated `Attachment`.
/// `filename` and `content` (mapped to `url`) are required — the entry is
/// skipped (returns `None`) when either is missing or not a string.
/// `mimeType` and `size` are optional.
fn parse_attachment_entry(entry: &serde_json::Value) -> Option<Attachment> {
    let filename = entry.get("filename")?.as_str()?.to_string();
    let url = entry.get("content")?.as_str()?.to_string();
    let mime_type = entry
        .get("mimeType")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let size = entry.get("size").and_then(|v| v.as_u64());
    Some(Attachment {
        filename,
        url,
        mime_type,
        size,
    })
}

/// Extract `values[]` from a raw `/project/search` response body into curated
/// `ProjectRow`s, mirroring `extract_attachments`'s raw-field-access pattern.
/// Absent, `null`, or non-array `values` yields an empty vec; each array entry
/// is parsed by `parse_project_entry`, which skips (never errors on) an entry
/// missing its required `key`/`name`.
fn extract_project_rows(raw: &serde_json::Value) -> Vec<ProjectRow> {
    raw.get("values")
        .and_then(|v| v.as_array())
        .map(|entries| entries.iter().filter_map(parse_project_entry).collect())
        .unwrap_or_default()
}

/// Parse a single raw `/project/search` `values[]` entry into a curated
/// `ProjectRow`. `key` and `name` are both required — the entry is skipped
/// (returns `None`) when either is missing or not a string.
fn parse_project_entry(entry: &serde_json::Value) -> Option<ProjectRow> {
    let key = entry.get("key")?.as_str()?.to_string();
    let name = entry.get("name")?.as_str()?.to_string();
    Some(ProjectRow { key, name })
}

fn map_comments(raw: &gouqi::Issue) -> Vec<IssueComment> {
    raw.comments()
        .map(|c| c.comments.into_iter().map(map_single_comment).collect())
        .unwrap_or_default()
}

fn map_single_comment(comment: gouqi::Comment) -> IssueComment {
    let author_account_id = comment.author.as_ref().and_then(|u| u.account_id.clone());
    IssueComment {
        id: comment.id,
        author: comment.author.map(|u| u.display_name),
        author_account_id,
        body: comment.body.to_string(),
        created: comment.created.map(|dt| dt.to_string()),
        updated: comment.updated.map(|dt| dt.to_string()),
    }
}

#[cfg(test)]
#[path = "../tests/unit/client.rs"]
mod tests;
