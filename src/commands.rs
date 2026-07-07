use crate::agent_json::{
    comment_result_err, comment_result_ok, issue_to_minified_json, mine_list_to_minified_json,
};
use crate::cli::extract_issue_key;
use crate::client::{ClientError, ClientResult, GouqiJiraClient, JiraClient};
use crate::i18n::SUPPORTED;
use crate::i18n::{t, tf};
use crate::render::{render_issue_human, render_issue_table};
use crate::store::cache::{IssueCache, TaskCache};
use crate::store::instances::{Instance, InstanceRepository};
use crate::store::settings::SettingsRepository;
use std::io::Write;

const DEFAULT_MINE_LIMIT: u64 = 50;
pub(crate) const DEFAULT_SEARCH_LIMIT: u64 = 50;
pub(crate) const MINE_JQL: &str =
    "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

/// Renders the actionable re-auth guidance for a 401 (ADR 0006: translate the
/// template, then substitute `{instance}`) — the single source shared by every
/// `Unauthorized` render site in CLI commands and the TUI shell.
pub(crate) fn reauth_message(instance: &str) -> String {
    tf(
        "Authentication failed for {instance}: your API token was rejected. Run `jira setup add` to re-authenticate.",
        &[("instance", instance)],
    )
}

pub fn pick_instance(
    instances: &[Instance],
    name: Option<&str>,
    err: &mut dyn Write,
) -> Result<usize, i32> {
    if instances.is_empty() {
        writeln!(
            err,
            "{}",
            t("Error: no instances configured. Run: jira setup add")
        )
        .ok();
        return Err(2);
    }

    if let Some(n) = name {
        match instances.iter().position(|i| i.name == n) {
            Some(idx) => return Ok(idx),
            None => {
                let known: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let known_str = known.join(", ");
                writeln!(
                    err,
                    "{}",
                    tf(
                        "Error: instance '{name}' not found. Known: {known}",
                        &[("name", n), ("known", &known_str)]
                    )
                )
                .ok();
                return Err(2);
            }
        }
    }

    if instances.len() == 1 {
        return Ok(0);
    }

    let names: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
    let names_str = names.join(", ");
    writeln!(
        err,
        "{}",
        tf(
            "Error: multiple instances configured ({names}). Use --instance NAME.",
            &[("names", &names_str)]
        )
    )
    .ok();
    Err(2)
}

pub fn setup_list(repo: &InstanceRepository<'_>, out: &mut dyn Write) -> i32 {
    let rows = match repo.list_for_display() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading instances: {e}");
            return 1;
        }
    };

    if rows.is_empty() {
        writeln!(out, "{}", t("No instances configured. Run: jira setup add")).ok();
        return 0;
    }

    writeln!(
        out,
        "{:<20} {:<40} {:<30} ACCOUNT_ID",
        "NAME", "URL", "EMAIL"
    )
    .ok();
    writeln!(out, "{}", "-".repeat(100)).ok();
    for (name, base_url, email, account_id) in &rows {
        let aid_str = account_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "{:<20} {:<40} {:<30} {}",
            name, base_url, email, aid_str
        )
        .ok();
    }
    0
}

pub fn setup_remove(
    repo: &InstanceRepository<'_>,
    cache: &TaskCache<'_>,
    name: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let deleted = match repo.delete(name) {
        Ok(n) => n,
        Err(e) => {
            writeln!(err, "Error deleting instance: {e}").ok();
            return 1;
        }
    };
    cache.delete_for_instance(name).ok();

    if deleted == 0 {
        writeln!(
            err,
            "{}",
            tf("Error: instance '{name}' not found.", &[("name", name)])
        )
        .ok();
        return 2;
    }
    writeln!(
        out,
        "{}",
        tf("Instance '{name}' removed.", &[("name", name)])
    )
    .ok();
    0
}

pub fn setup_language(
    settings: &SettingsRepository<'_>,
    code: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match code {
        None => {
            let current = settings
                .get("language", Some("en"))
                .unwrap_or(Some("en".to_owned()))
                .unwrap_or_else(|| "en".to_owned());
            writeln!(
                out,
                "{}",
                tf("Current language: {code}", &[("code", &current)])
            )
            .ok();
            0
        }
        Some(c) => match crate::i18n::normalize_locale(c) {
            Some(canon) => {
                if let Err(e) = settings.set("language", canon) {
                    writeln!(err, "Error saving language: {e}").ok();
                    return 1;
                }
                writeln!(
                    out,
                    "{}",
                    tf("Language set to '{code}'.", &[("code", canon)])
                )
                .ok();
                0
            }
            None => {
                let supported = SUPPORTED.join(", ");
                writeln!(
                    err,
                    "{}",
                    tf(
                        "Error: unsupported language '{code}'. Supported: {supported}.",
                        &[("code", c), ("supported", &supported)]
                    )
                )
                .ok();
                2
            }
        },
    }
}

pub async fn setup_test(
    repo: &InstanceRepository<'_>,
    name: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let rows = match name {
        Some(n) => {
            let found = match repo.find_by_name(n) {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "Error querying instances: {e}").ok();
                    return 1;
                }
            };
            if found.is_empty() {
                writeln!(
                    err,
                    "{}",
                    tf("Error: instance '{name}' not found.", &[("name", n)])
                )
                .ok();
                return 2;
            }
            found
        }
        None => match repo.list_connectivity() {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "Error querying instances: {e}").ok();
                return 1;
            }
        },
    };

    let mut any_failed = false;
    for (inst_name, base_url, token) in &rows {
        let temp_instance = Instance {
            name: inst_name.clone(),
            base_url: base_url.clone(),
            email: String::new(),
            token: token.clone(),
            account_id: None,
        };
        match verify_connectivity(&temp_instance).await {
            Ok(_) => {
                writeln!(out, "  {inst_name}: OK").ok();
            }
            Err(status) => {
                writeln!(out, "  {inst_name}: FAILED (HTTP {status})").ok();
                any_failed = true;
            }
        }
    }
    if any_failed {
        1
    } else {
        0
    }
}

/// Verifies connectivity to a Jira instance by calling /rest/api/3/myself.
/// Returns Ok(account_id) on success, Err(http_status_or_description) on failure.
async fn verify_connectivity(instance: &Instance) -> Result<String, String> {
    let client = GouqiJiraClient::new(instance).map_err(|e| format!("client build error: {e}"))?;
    client.myself().await.map(|me| me.account_id).map_err(|e| {
        let msg = e.to_string();
        extract_http_status(&msg).unwrap_or(msg)
    })
}

/// Extract HTTP status code string from a gouqi/anyhow error message.
/// gouqi may format errors as "... Unauthorized ..." (reason phrase) or "... 401 ..."
fn extract_http_status(msg: &str) -> Option<String> {
    // First try numeric status codes in the message
    for token in msg.split_whitespace() {
        let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(code) = digits.parse::<u16>() {
            if (100..=599).contains(&code) {
                return Some(code.to_string());
            }
        }
    }
    // Fall back to HTTP reason-phrase mapping (gouqi Cloud errors use these)
    let lower = msg.to_lowercase();
    let phrase_map: &[(&str, &str)] = &[
        ("unauthorized", "401"),
        ("forbidden", "403"),
        ("not found", "404"),
        ("bad request", "400"),
        ("internal server error", "500"),
        ("bad gateway", "502"),
        ("service unavailable", "503"),
    ];
    for (phrase, code) in phrase_map {
        if lower.contains(phrase) {
            return Some((*code).to_string());
        }
    }
    None
}

pub struct SetupAddFields {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

pub async fn setup_add(
    fields: SetupAddFields,
    token: Option<String>,
    repo: &InstanceRepository<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (name, url, email) = match (fields.name, fields.url, fields.email) {
        (Some(n), Some(u), Some(e)) if !n.is_empty() && !u.is_empty() && !e.is_empty() => (n, u, e),
        _ => {
            writeln!(
                err,
                "{}",
                t("Error: --name, --url and --email are required.")
            )
            .ok();
            return 2;
        }
    };

    let api_token = match token {
        Some(t) if !t.is_empty() => t,
        _ => {
            writeln!(
                err,
                "{}",
                t("Error: --name, --url and --email are required.")
            )
            .ok();
            return 2;
        }
    };

    let probe = Instance {
        name: name.clone(),
        base_url: url.clone(),
        email: email.clone(),
        token: api_token.clone(),
        account_id: None,
    };

    let account_id = match verify_connectivity(&probe).await {
        Ok(aid) => aid,
        Err(status_or_msg) => {
            writeln!(out, "Connectivity: FAILED (HTTP {status_or_msg})").ok();
            return 1;
        }
    };

    let instance = Instance {
        name: name.clone(),
        base_url: url,
        email,
        token: api_token,
        account_id: Some(account_id),
    };

    if let Err(e) = repo.save(&instance) {
        writeln!(err, "Error saving instance: {e}").ok();
        return 1;
    }

    writeln!(out, "Instance '{name}' saved.").ok();
    writeln!(out, "Connectivity: OK").ok();
    0
}

/// Parse a bare issue key (PROJ-123) or a Jira browse URL into just the issue key.
/// Returns None if the ref is not a recognised format.
pub fn parse_issue_ref(ref_: &str) -> Option<String> {
    let trimmed = ref_.trim();
    // Browse URL: https://<site>.atlassian.net/browse/PROJ-123
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return extract_key_from_url(trimmed);
    }
    // Bare key: one or more uppercase letters, a dash, one or more digits.
    if is_issue_key(trimmed) {
        return Some(trimmed.to_string());
    }
    None
}

fn extract_key_from_url(url: &str) -> Option<String> {
    let path_start = url.find("/browse/")?;
    let after_browse = &url[path_start + "/browse/".len()..];
    // Key ends at the next '/' or '?' or end of string.
    let key_end = after_browse.find(['/', '?']).unwrap_or(after_browse.len());
    let candidate = &after_browse[..key_end];
    if is_issue_key(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn is_issue_key(s: &str) -> bool {
    let Some(dash_pos) = s.rfind('-') else {
        return false;
    };
    let (prefix, number) = (&s[..dash_pos], &s[dash_pos + 1..]);
    !prefix.is_empty()
        && !number.is_empty()
        && prefix.chars().all(|c| c.is_ascii_uppercase() || c == '-')
        && number.chars().all(|c| c.is_ascii_digit())
}

/// Display and fetch options for `get_core`.
pub struct GetOpts {
    pub json: bool,
    pub no_comments: bool,
    pub refresh: bool,
}

pub async fn get_core(
    ref_: &str,
    instance: &Instance,
    cache: &TaskCache<'_>,
    opts: GetOpts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let issue_key = match parse_issue_ref(ref_) {
        Some(k) => k,
        None => {
            writeln!(
                err,
                "Error: '{}' is not a valid issue key or Jira browse URL.",
                ref_
            )
            .ok();
            return 2;
        }
    };

    let issue_cache = IssueCache::new(cache.conn());
    let issue = match load_issue(&issue_key, instance, &issue_cache, opts.refresh, err).await {
        Ok(i) => i,
        Err(code) => return code,
    };

    if opts.json {
        let line =
            issue_to_minified_json(&issue, &instance.name, &instance.base_url, opts.no_comments);
        writeln!(out, "{line}").ok();
    } else {
        render_issue_human(
            &issue,
            &instance.name,
            &instance.base_url,
            opts.no_comments,
            out,
        );
    }

    0
}

/// Resolve an issue: serve from cache when available and refresh is not requested,
/// otherwise fetch from the network and update the cache on success.
///
/// A cache read error is treated as a miss so a corrupt/locked cache never
/// prevents the command from running; the fetch path recovers transparently.
pub(crate) async fn load_issue(
    issue_key: &str,
    instance: &Instance,
    issue_cache: &IssueCache<'_>,
    refresh: bool,
    err: &mut dyn Write,
) -> Result<crate::models::Issue, i32> {
    if !refresh {
        match issue_cache.read(&instance.name, issue_key) {
            Ok(Some(cached)) => return Ok(cached.issue),
            Ok(None) => {}
            Err(e) => {
                writeln!(
                    err,
                    "Warning: cache read error (falling through to fetch): {e}"
                )
                .ok();
            }
        }
    }

    fetch_and_cache(issue_key, instance, issue_cache, err).await
}

async fn fetch_and_cache(
    issue_key: &str,
    instance: &Instance,
    issue_cache: &IssueCache<'_>,
    err: &mut dyn Write,
) -> Result<crate::models::Issue, i32> {
    let client = match GouqiJiraClient::new(instance) {
        Ok(c) => c,
        Err(e) => {
            writeln!(err, "Error building client: {e}").ok();
            return Err(1);
        }
    };

    let issue = match client.get_issue(issue_key).await {
        Ok(i) => i,
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(err, "{}", reauth_message(&instance)).ok();
            return Err(1);
        }
        Err(e) => {
            let msg = e.to_string();
            if is_not_found_error(&msg) {
                writeln!(err, "Error: issue '{}' not found.", issue_key).ok();
            } else {
                writeln!(err, "Error fetching issue '{}': {}", issue_key, msg).ok();
            }
            return Err(1);
        }
    };

    if let Err(e) = issue_cache.write(&instance.name, &issue) {
        writeln!(err, "Warning: failed to write cache: {e}").ok();
    }

    Ok(issue)
}

fn is_not_found_error(msg: &str) -> bool {
    msg.contains("404") || msg.to_lowercase().contains("not found")
}

pub async fn current_core(
    branch: Option<&str>,
    instance: &Instance,
    cache: &TaskCache<'_>,
    opts: GetOpts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let branch_name = match branch {
        None => {
            writeln!(
                err,
                "{}",
                t("Error: not in a git repository / no current branch.")
            )
            .ok();
            return 2;
        }
        Some(b) => b,
    };
    match extract_issue_key(branch_name) {
        None => {
            writeln!(err, "Error: no issue key in branch '{branch_name}'.").ok();
            2
        }
        Some(key) => get_core(&key, instance, cache, opts, out, err).await,
    }
}

pub async fn mine_core(
    repo: &InstanceRepository<'_>,
    instance_filter: Option<&str>,
    json: bool,
    limit: Option<u64>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            writeln!(err, "Error loading instances: {e}").ok();
            return 1;
        }
    };

    let idx = match pick_instance(&instances, instance_filter, err) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let instance = &instances[idx];

    let client = match GouqiJiraClient::new(instance) {
        Ok(c) => c,
        Err(e) => {
            writeln!(err, "Error building client: {e}").ok();
            return 1;
        }
    };

    let max_results = limit.unwrap_or(DEFAULT_MINE_LIMIT);
    let result = match client.search(MINE_JQL, max_results).await {
        Ok(r) => r,
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(err, "{}", reauth_message(&instance)).ok();
            return 1;
        }
        Err(e) => {
            writeln!(err, "Error fetching issues: {e}").ok();
            return 1;
        }
    };

    render_mine_output(json, MINE_JQL, &result.issues, out);
    0
}

fn render_mine_output(
    json: bool,
    jql: &str,
    issues: &[crate::models::IssueRow],
    out: &mut dyn Write,
) {
    if json {
        let line = mine_list_to_minified_json(jql, issues);
        writeln!(out, "{line}").ok();
    } else if issues.is_empty() {
        writeln!(out, "{}", t("No issues.")).ok();
    } else {
        render_issue_table(out, issues);
    }
}

fn search_error_message(err_msg: &str) -> Option<String> {
    match extract_http_status(err_msg) {
        Some(code) if code == "400" => Some(err_msg.to_string()),
        _ => {
            let lower = err_msg.to_lowercase();
            if lower.contains("bad request") {
                Some(err_msg.to_string())
            } else {
                None
            }
        }
    }
}

pub async fn search_core(
    jql: Option<&str>,
    instance: &Instance,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let trimmed = jql.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        writeln!(err, "{}", t("Error: search requires a JQL query.")).ok();
        return 2;
    }

    let client = match GouqiJiraClient::new(instance) {
        Ok(c) => c,
        Err(e) => {
            writeln!(err, "Error building client: {e}").ok();
            return 1;
        }
    };

    match client.search(trimmed, DEFAULT_SEARCH_LIMIT).await {
        Ok(result) => {
            render_mine_output(json, trimmed, &result.issues, out);
            0
        }
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(err, "{}", reauth_message(&instance)).ok();
            1
        }
        Err(e) => {
            let msg = e.to_string();
            match search_error_message(&msg) {
                Some(_) => {
                    writeln!(err, "invalid JQL: {msg}").ok();
                }
                None => {
                    writeln!(err, "Error running search: {e}").ok();
                }
            }
            1
        }
    }
}

const NO_ISSUE_KEY_MSG: &str =
    "Error: no issue key. Provide ISSUE_KEY, or run from a branch containing one.";

/// Body source resolved by the dispatcher before calling into `comment_core`
/// (ADR 0023 §2): `-m` wins, otherwise piped stdin text, otherwise nothing —
/// kept as data so the core stays testable without a real terminal.
pub enum CommentBody {
    Flag(String),
    Piped(String),
    None,
}

/// One-shot, non-interactive comment post (BDR 0014). Resolves the issue key
/// (explicit arg, else the branch's key via the same extraction `current`
/// uses) and the body (`CommentBody`), then posts through the C1 write seam
/// (`client.add_comment`). Never a false success: any error path returns
/// non-zero with no confirmation line.
pub async fn comment_core(
    issue_key: Option<&str>,
    branch: Option<&str>,
    body: CommentBody,
    instance: &Instance,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let key = match resolve_comment_key(issue_key, branch) {
        Some(k) => k,
        None => return report_comment_usage_error(json, &t(NO_ISSUE_KEY_MSG), out, err),
    };

    let comment_body = match resolve_comment_body(body) {
        Some(b) => b,
        None => return report_comment_usage_error(json, &t("no comment body"), out, err),
    };

    let client = match GouqiJiraClient::new(instance) {
        Ok(c) => c,
        Err(e) => {
            writeln!(err, "Error building client: {e}").ok();
            return 1;
        }
    };

    let result = client.add_comment(&key, &comment_body).await;
    handle_add_comment_result(result, &key, json, out, err)
}

/// Explicit `issue_key` wins; otherwise fall back to the branch's key via the
/// same extraction `current_core` uses — never a second implementation.
fn resolve_comment_key(issue_key: Option<&str>, branch: Option<&str>) -> Option<String> {
    issue_key
        .map(str::to_owned)
        .or_else(|| branch.and_then(extract_issue_key))
}

/// An empty body (from either channel) is a usage error, not an empty write.
fn resolve_comment_body(body: CommentBody) -> Option<String> {
    let text = match body {
        CommentBody::Flag(s) | CommentBody::Piped(s) => s,
        CommentBody::None => return None,
    };
    (!text.trim().is_empty()).then_some(text)
}

fn report_comment_usage_error(
    json: bool,
    message: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if json {
        writeln!(out, "{}", comment_result_err(message)).ok();
    } else {
        writeln!(err, "{message}").ok();
    }
    2
}

fn handle_add_comment_result(
    result: ClientResult<crate::models::CommentWriteResult>,
    key: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match result {
        Ok(r) => {
            report_comment_success(json, &r.id, key, out);
            0
        }
        Err(ClientError::Unauthorized { instance }) => {
            writeln!(err, "{}", reauth_message(&instance)).ok();
            1
        }
        Err(ClientError::Other(e)) => {
            report_comment_failure(json, key, &e.to_string(), out, err);
            1
        }
    }
}

fn report_comment_success(json: bool, comment_id: &str, key: &str, out: &mut dyn Write) {
    if json {
        writeln!(out, "{}", comment_result_ok(comment_id, key)).ok();
    } else {
        writeln!(out, "{}", tf("Comment added to {key}.", &[("key", key)])).ok();
    }
}

fn report_comment_failure(
    json: bool,
    key: &str,
    message: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    if json {
        writeln!(out, "{}", comment_result_err(message)).ok();
    } else {
        writeln!(err, "Error posting comment to '{key}': {message}").ok();
    }
}

#[cfg(test)]
#[path = "../tests/unit/commands.rs"]
mod tests;
