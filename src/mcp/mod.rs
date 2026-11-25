//! MCP server over stdio. Three tools, `jira_read`, `jira_write` and
//! `jira_download_attachments`, select a command core by `operation` and return
//! the core's `--json` output, so the agent contract stays the one in
//! `agent_json`. While the server runs, stdout belongs to the protocol: every
//! core writes to in-memory buffers.

use crate::commands::{
    comment_core, current_core, get_core, mine_core, search_core_limited, CommentBody, GetOpts,
    DEFAULT_SEARCH_LIMIT,
};
use crate::store::cache::TaskCache;
use crate::store::instances::InstanceRepository;
use crate::{
    current_git_branch, download_core, issue_key_or_report, open_store_to,
    resolve_single_instance_to, ResolvedInstance,
};
use rmcp::handler::server::common::{schema_for_empty_input, schema_for_input};
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

/// Serve MCP on stdin/stdout until the client closes the connection.
pub(crate) async fn serve() -> i32 {
    let running = match JiraServer.serve(stdio()).await {
        Ok(running) => running,
        Err(e) => {
            eprintln!("Error starting MCP server: {e}");
            return 1;
        }
    };
    match running.waiting().await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error serving MCP: {e}");
            1
        }
    }
}

#[derive(Clone, Copy)]
struct JiraServer;

/// Identifies the server as `jira` at this crate's version in `initialize`.
#[tool_handler(name = "jira")]
impl ServerHandler for JiraServer {}

#[tool_router]
impl JiraServer {
    #[tool(
        description = "Read Jira data as the `jira ... --json` commands print it.",
        input_schema = input_schema::<ReadOperation>(),
        annotations(read_only_hint = true)
    )]
    async fn jira_read(&self, arguments: JsonObject) -> Result<CallToolResult, McpError> {
        let operation: ReadOperation = parse_operation(arguments)?;
        run_isolated(move || run_read(operation)).await
    }

    #[tool(
        description = "Change Jira data (post a comment).",
        input_schema = input_schema::<WriteOperation>(),
        annotations(destructive_hint = true)
    )]
    async fn jira_write(&self, arguments: JsonObject) -> Result<CallToolResult, McpError> {
        let operation: WriteOperation = parse_operation(arguments)?;
        run_isolated(move || run_write(operation)).await
    }

    #[tool(
        description = "Download every attachment of one issue into a local directory. Writes \
                       files on the server's machine; it changes nothing in Jira.",
        input_schema = input_schema::<DownloadOperation>(),
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true)
    )]
    async fn jira_download_attachments(
        &self,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let operation: DownloadOperation = parse_operation(arguments)?;
        run_isolated(move || run_download(operation)).await
    }
}

/// The tool's input schema. A tagged enum derives as a top-level `oneOf`,
/// which Claude's tool API rejects, so the variants are merged into one flat
/// object (see `flatten_operations`).
fn input_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    let mut schema = schema_for_input::<T>().unwrap_or_else(|_| schema_for_empty_input());
    let published = Arc::make_mut(&mut schema);
    published.remove("$schema");
    flatten_operations(published);
    schema
}

/// Rewrite a schema whose root is a `oneOf` of tagged variants into one object:
/// an `operation` string enum plus the union of every variant's properties,
/// each described with the operations that take it. A schema without a
/// `oneOf` is left as it is. A tool with a single operation repeats nothing:
/// its fields carry no takers note and `operation` keeps the operation's own
/// doc. Descriptions are collapsed to one line. Only the published schema
/// changes; the enum's own parse still rejects a field the chosen operation
/// does not take.
fn flatten_operations(schema: &mut JsonObject) {
    let Some(Value::Array(variants)) = schema.remove("oneOf") else {
        return;
    };
    let mut operations: Vec<Value> = Vec::new();
    let mut properties = serde_json::Map::new();
    let mut takers: Vec<(String, Vec<String>)> = Vec::new();
    let mut summaries: Vec<String> = Vec::new();
    for variant in &variants {
        if let Some(name) = variant["properties"]["operation"]["const"].as_str() {
            operations.push(Value::String(name.to_owned()));
            summaries.push(match variant["description"].as_str() {
                Some(doc) => format!("{name}: {doc}"),
                None => name.to_owned(),
            });
            merge_variant(name, variant, &mut properties, &mut takers);
        }
    }
    let single = operations.len() == 1;
    describe_fields(&mut properties, takers, single);
    let description = operation_description(&summaries, single);
    let mut flat = serde_json::Map::new();
    flat.insert(
        "operation".into(),
        serde_json::json!({
            "type": "string",
            "enum": operations,
            "description": description
        }),
    );
    flat.extend(properties);
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(flat));
    schema.insert("required".into(), serde_json::json!(["operation"]));
}

fn describe_fields(
    properties: &mut serde_json::Map<String, Value>,
    takers: Vec<(String, Vec<String>)>,
    single: bool,
) {
    for (field, labels) in takers {
        if let Some(Value::Object(field_schema)) = properties.get_mut(&field) {
            if !single {
                annotate_takers(field_schema, &labels);
            } else if labels.iter().any(|label| label.ends_with("(required)")) {
                mark_required(field_schema);
            }
            collapse_description(field_schema);
        }
    }
}

fn operation_description(summaries: &[String], single: bool) -> String {
    match summaries {
        [only] if single => only
            .split_once(": ")
            .map_or(only.clone(), |(_, doc)| doc.to_owned()),
        _ => format!("The command to run. {}", summaries.join(" ")),
    }
}

/// Add one variant's fields (all but the `operation` tag) to the merged
/// properties, and `takers` records which operations take it and whether they
/// require it. Operations sharing a field must declare the same schema for it,
/// since only one can be published.
fn merge_variant(
    name: &str,
    variant: &Value,
    properties: &mut serde_json::Map<String, Value>,
    takers: &mut Vec<(String, Vec<String>)>,
) {
    let required: Vec<&str> = variant["required"]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let fields = variant["properties"].as_object().into_iter().flatten();
    for (field, field_schema) in fields.filter(|(field, _)| *field != "operation") {
        let published = properties
            .entry(field.clone())
            .or_insert_with(|| field_schema.clone());
        assert_eq!(
            published, field_schema,
            "operation {name} declares field {field} differently from an earlier operation"
        );
        let label = if required.contains(&field.as_str()) {
            format!("{name} (required)")
        } else {
            name.to_owned()
        };
        match takers.iter_mut().find(|(taken, _)| taken == field) {
            Some((_, labels)) => labels.push(label),
            None => takers.push((field.clone(), vec![label])),
        }
    }
}

fn mark_required(field_schema: &mut serde_json::Map<String, Value>) {
    let text = match field_schema.get("description").and_then(Value::as_str) {
        Some(doc) => format!("{doc} (required)"),
        None => "(required)".to_owned(),
    };
    field_schema.insert("description".into(), Value::String(text));
}

fn collapse_description(field_schema: &mut serde_json::Map<String, Value>) {
    if let Some(Value::String(text)) = field_schema.get_mut("description") {
        *text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    }
}

fn annotate_takers(field_schema: &mut serde_json::Map<String, Value>, labels: &[String]) {
    let note = format!("Taken by: {}.", labels.join(", "));
    let text = match field_schema.get("description").and_then(Value::as_str) {
        Some(doc) => format!("{doc} {note}"),
        None => note,
    };
    field_schema.insert("description".into(), Value::String(text));
}

/// Parse the tool arguments by hand: rmcp reports a failed `Parameters`
/// parse as a tool error, while an unknown operation or a missing field is a
/// malformed request, so it is answered as the `invalid_params` protocol error.
fn parse_operation<T: DeserializeOwned>(arguments: JsonObject) -> Result<T, McpError> {
    serde_json::from_value(Value::Object(arguments))
        .map_err(|e| McpError::invalid_params(format!("invalid arguments: {e}"), None))
}

/// `get` and `current` read one issue, `mine` lists the open issues assigned
/// to you, `search` runs a JQL query. Every operation takes an optional
/// `instance` name, needed only when several instances are configured.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(extend("type" = "object"))]
enum ReadOperation {
    /// Fetch one issue with its comments.
    Get {
        /// Issue key or Jira browse URL.
        #[serde(rename = "ref")]
        issue_ref: String,
        #[serde(default)]
        no_comments: bool,
        /// Ignore the cache and re-fetch.
        #[serde(default)]
        refresh: bool,
        instance: Option<String>,
    },
    /// Fetch the issue of a git branch whose name contains an issue key.
    Current {
        /// Defaults to the git branch of the server's working directory.
        branch: Option<String>,
        instance: Option<String>,
    },
    /// List open issues assigned to you.
    Mine {
        /// Maximum number of issues to return.
        limit: Option<u64>,
        instance: Option<String>,
    },
    /// Run a JQL query.
    Search {
        /// JQL query string.
        jql: String,
        /// Maximum number of issues to return.
        limit: Option<u64>,
        instance: Option<String>,
    },
}

/// `comment` posts to one issue.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(extend("type" = "object"))]
enum WriteOperation {
    /// Post a comment as the logged-in user.
    Comment {
        /// Issue key or Jira browse URL; defaults to the issue key in the git
        /// branch of the server's working directory.
        #[serde(rename = "ref")]
        issue_ref: Option<String>,
        /// Comment text, posted verbatim.
        body: String,
        instance: Option<String>,
    },
}

/// `download` saves every attachment of one issue.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(extend("type" = "object"))]
enum DownloadOperation {
    /// Save the attachments of one issue to disk.
    Download {
        /// Issue key or Jira browse URL.
        #[serde(rename = "ref")]
        issue_ref: String,
        /// Destination directory; defaults to a per-issue directory under the
        /// jira config directory.
        directory: Option<String>,
        instance: Option<String>,
    },
}

/// What a command core produced: its exit code and its two writers.
#[derive(Default)]
struct CoreRun {
    code: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl CoreRun {
    /// Exit 0 returns `out`; any other exit is a tool error carrying `err`,
    /// or `out` when the core reports a failure there as JSON.
    fn into_result(self) -> CallToolResult {
        if self.code == 0 {
            return CallToolResult::success(vec![ContentBlock::text(lossy(self.out))]);
        }
        let message = if self.err.is_empty() {
            lossy(self.out)
        } else {
            lossy(self.err)
        };
        CallToolResult::error(vec![ContentBlock::text(message)])
    }
}

fn lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Run a core on a blocking thread. The cores hold a borrowed SQLite
/// connection across awaits, which is not `Send`, so the future is driven to
/// completion by `block_on` on that thread instead of being spawned.
async fn run_isolated<F, Fut>(make: F) -> Result<CallToolResult, McpError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = CoreRun>,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || handle.block_on(make()))
        .await
        .map(CoreRun::into_result)
        .map_err(|e| McpError::internal_error(format!("tool task failed: {e}"), None))
}

fn resolve(instance: Option<&str>, run: &mut CoreRun) -> Option<ResolvedInstance> {
    match resolve_single_instance_to(instance, &mut run.err) {
        Ok(resolved) => Some(resolved),
        Err(code) => {
            run.code = code;
            None
        }
    }
}

async fn run_read(operation: ReadOperation) -> CoreRun {
    let mut run = CoreRun::default();
    match operation {
        ReadOperation::Get {
            issue_ref,
            no_comments,
            refresh,
            instance,
        } => {
            let Some(ResolvedInstance { store, instance }) = resolve(instance.as_deref(), &mut run)
            else {
                return run;
            };
            let cache = TaskCache::new(store.conn());
            run.code = get_core(
                &issue_ref,
                &instance,
                &cache,
                GetOpts {
                    json: true,
                    no_comments,
                    refresh,
                },
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        ReadOperation::Current { branch, instance } => {
            let Some(ResolvedInstance { store, instance }) = resolve(instance.as_deref(), &mut run)
            else {
                return run;
            };
            let cache = TaskCache::new(store.conn());
            let branch = branch.or_else(current_git_branch);
            run.code = current_core(
                branch.as_deref(),
                &instance,
                &cache,
                GetOpts {
                    json: true,
                    no_comments: false,
                    refresh: false,
                },
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        ReadOperation::Mine { limit, instance } => {
            let Some(store) = open_store_to(&mut run.err) else {
                run.code = 1;
                return run;
            };
            let repo = InstanceRepository::new(store.conn());
            run.code = mine_core(
                &repo,
                instance.as_deref(),
                true,
                limit,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        ReadOperation::Search {
            jql,
            limit,
            instance,
        } => {
            let Some(ResolvedInstance { instance, .. }) = resolve(instance.as_deref(), &mut run)
            else {
                return run;
            };
            run.code = search_core_limited(
                Some(&jql),
                &instance,
                limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
                true,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
    }
    run
}

async fn run_write(operation: WriteOperation) -> CoreRun {
    let mut run = CoreRun::default();
    match operation {
        WriteOperation::Comment {
            issue_ref,
            body,
            instance,
        } => {
            let Some(ResolvedInstance { instance, .. }) = resolve(instance.as_deref(), &mut run)
            else {
                return run;
            };
            let branch = match issue_ref {
                Some(_) => None,
                None => current_git_branch(),
            };
            run.code = comment_core(
                issue_ref.as_deref(),
                branch.as_deref(),
                CommentBody::Flag(body),
                &instance,
                true,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
    }
    run
}

async fn run_download(operation: DownloadOperation) -> CoreRun {
    let mut run = CoreRun::default();
    let DownloadOperation::Download {
        issue_ref,
        directory,
        instance,
    } = operation;
    let key = match issue_key_or_report(&issue_ref, &mut run.err) {
        Ok(key) => key,
        Err(code) => {
            run.code = code;
            return run;
        }
    };
    let Some(ResolvedInstance { instance, .. }) = resolve(instance.as_deref(), &mut run) else {
        return run;
    };
    run.code = download_core(
        &key,
        &instance,
        directory.as_deref().map(Path::new),
        true,
        &mut run.out,
        &mut run.err,
    )
    .await;
    run
}

#[cfg(test)]
#[path = "../../tests/unit/mcp.rs"]
mod tests;
