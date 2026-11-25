use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams, ClientCapabilities,
    ClientConfig, ErrorCode, GetTaskParams, Implementation, ProgressNotificationParam,
    ProgressToken,
};
use rmcp::service::{NotificationContext, RoleClient, RunningService, ServiceError};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, Peer, ServiceExt};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

type Client = RunningService<RoleClient, ()>;
type TaskClient = RunningService<RoleClient, TaskingClient>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<ProgressToken>;

struct TaskingClient {
    progress: tokio::sync::mpsc::UnboundedSender<ProgressToken>,
}

impl ClientHandler for TaskingClient {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::builder().enable_tasks().build(),
            Implementation::new("tasking-test-client", "0"),
        )
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        let _ = self.progress.send(params.progress_token);
    }
}

const ONE: &str = concat!("ACME", "-1");
const TWO: &str = concat!("ACME", "-2");
const MISSING: &str = concat!("ACME", "-404");
const DENIED: &str = concat!("ACME", "-401");
const ROW_A: &str = concat!("ACME", "-10");
const ROW_B: &str = concat!("ACME", "-11");
const BRANCH: &str = concat!("feature/", "ACME", "-1-login");
const ATTACHMENT_BODY: &str = "attachment bytes";

fn issue_payload(key: &str, base: &str) -> Value {
    json!({
        "id": "10001",
        "key": key,
        "self": format!("{base}/rest/api/3/issue/10001"),
        "fields": {
            "summary": "Fix the login bug",
            "status": {
                "id": "3",
                "name": "In Progress",
                "description": "",
                "iconUrl": "",
                "self": "",
                "statusCategory": { "id": 4, "key": "indeterminate", "colorName": "yellow", "name": "In Progress" }
            },
            "issuetype": {
                "id": "10002",
                "name": "Bug",
                "description": "",
                "iconUrl": "",
                "self": "",
                "subtask": false
            },
            "assignee": {
                "accountId": "acc-alice",
                "displayName": "Alice",
                "active": true,
                "self": "",
                "avatarUrls": {}
            },
            "priority": { "id": "2", "name": "High", "iconUrl": "", "self": "" },
            "created": "2026-01-10T09:00:00.000+0000",
            "updated": "2026-06-29T12:00:00.000+0000",
            "description": {
                "type": "doc",
                "version": 1,
                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Login fails with MFA." }] }]
            },
            "comment": {
                "self": format!("{base}/rest/api/3/issue/10001/comment"),
                "comments": [{
                    "id": "100",
                    "self": "",
                    "author": { "accountId": "acc-bob", "displayName": "Bob", "active": true, "self": "", "avatarUrls": {} },
                    "body": "Reproduced on v2.1.",
                    "created": "2026-06-29T10:00:00.000+0000",
                    "updated": "2026-06-29T10:00:00.000+0000"
                }],
                "maxResults": 1,
                "total": 1,
                "startAt": 0
            },
            "attachment": [
                {
                    "id": "200",
                    "filename": "screenshot.png",
                    "content": format!("{base}/secure/attachment/200/screenshot.png"),
                    "mimeType": "image/png",
                    "size": 16
                },
                {
                    "id": "201",
                    "filename": "notes.txt",
                    "content": format!("{base}/secure/attachment/201/notes.txt")
                }
            ]
        }
    })
}

fn search_payload() -> Value {
    let row = |key: &str, summary: &str| {
        json!({
            "id": "10001",
            "key": key,
            "self": "",
            "fields": {
                "summary": summary,
                "status": {
                    "id": "1", "name": "Open", "description": "", "iconUrl": "", "self": "",
                    "statusCategory": { "id": 2, "key": "new", "colorName": "blue-gray", "name": "To Do" }
                },
                "issuetype": { "id": "10002", "name": "Bug", "description": "", "iconUrl": "", "self": "", "subtask": false },
                "assignee": null,
                "priority": { "id": "3", "name": "Medium", "iconUrl": "", "self": "" },
                "created": "2026-01-01T00:00:00.000+0000",
                "updated": "2026-06-29T00:00:00.000+0000"
            }
        })
    };
    json!({
        "issues": [row(ROW_A, "Critical bug"), row(ROW_B, "Cleanup task")],
        "isLast": true,
        "nextPageToken": null
    })
}

struct Env {
    dir: TempDir,
    db: PathBuf,
    server: MockServer,
}

impl Env {
    async fn new() -> Env {
        let server = MockServer::start().await;
        let base = server.uri();
        for key in [ONE, TWO] {
            Mock::given(method("GET"))
                .and(path(format!("/rest/api/3/issue/{key}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(issue_payload(key, &base)))
                .mount(&server)
                .await;
        }
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/issue/{MISSING}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/issue/{DENIED}")))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/search/jql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_payload()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/secure/attachment/\d+/.+$"))
            .respond_with(ResponseTemplate::new(200).set_body_string(ATTACHMENT_BODY))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path_regex(r"^/rest/api/3/issue/ACME-\d+/comment$"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "9001" })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/myself"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "accountId": "acc-me",
                "displayName": "Me",
                "active": true,
                "self": "",
                "avatarUrls": {}
            })))
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let db = dir.path().join("jira.db");
        let env = Env { dir, db, server };
        env.register_instance();
        env.install_git_stub(BRANCH);
        env
    }

    fn command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_jira"));
        cmd.env("JIRA_DB", &self.db)
            .env("JIRA_CLI_LANG", "en")
            .env("HOME", self.dir.path())
            .env("PATH", self.path_with_git_stub())
            .current_dir(self.dir.path());
        cmd
    }

    fn register_instance(&self) {
        self.add_instance("work", &self.server.uri());
    }

    fn add_instance(&self, name: &str, url: &str) {
        let mut child = self
            .command()
            .args(["setup", "add", "--name", name, "--url"])
            .arg(url)
            .args(["--email", "me@example.com"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "api-token").unwrap();
        assert!(child.wait().unwrap().success(), "setup add failed");
    }

    /// A `git` stand-in on the PATH of every spawned process: the image that
    /// runs the suite ships no git, and the branch is what the tests need.
    fn install_git_stub(&self, branch: &str) {
        use std::os::unix::fs::PermissionsExt;
        let bin = self.dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("git");
        std::fs::write(&script, format!("#!/bin/sh\necho '{branch}'\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn path_with_git_stub(&self) -> String {
        let inherited = std::env::var("PATH").unwrap_or_default();
        format!("{}:{inherited}", self.dir.path().join("bin").display())
    }

    /// The CLI's stdout and stderr for `args`, with whether it exited 0.
    fn cli(&self, args: &[&str]) -> (bool, String, String) {
        let output = self.command().args(args).output().unwrap();
        (
            output.status.success(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn cli_stdout(&self, args: &[&str]) -> String {
        let (ok, stdout, stderr) = self.cli(args);
        assert!(ok, "jira {args:?} failed: {stderr}");
        stdout
    }

    fn mcp_transport(&self) -> TokioChildProcess {
        let cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_jira"));
        TokioChildProcess::new(cmd.configure(|c| {
            c.arg("mcp")
                .env("JIRA_DB", &self.db)
                .env("JIRA_CLI_LANG", "en")
                .env("HOME", self.dir.path())
                .env("PATH", self.path_with_git_stub())
                .current_dir(self.dir.path());
        }))
        .unwrap()
    }

    async fn connect(&self) -> Client {
        ().serve(self.mcp_transport()).await.unwrap()
    }

    /// A client that declares the tasks extension and reports the token of every progress
    /// notification it receives.
    async fn connect_declaring_tasks(&self) -> (TaskClient, Receiver) {
        let (progress, received) = tokio::sync::mpsc::unbounded_channel();
        let client = TaskingClient { progress }
            .serve(self.mcp_transport())
            .await
            .unwrap();
        (client, received)
    }

    /// Every attachment download answers only after `delay`.
    async fn delay_attachments(&self, delay: Duration) {
        Mock::given(method("GET"))
            .and(path_regex(r"^/secure/attachment/\d+/.+$"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(ATTACHMENT_BODY)
                    .set_delay(delay),
            )
            .with_priority(1)
            .mount(&self.server)
            .await;
    }

    async fn post_count(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.method.as_str() == "POST")
            .count()
    }
}

fn object(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("expected a JSON object, got {other}"),
    }
}

async fn call(
    client: &Client,
    tool: &'static str,
    args: Value,
) -> Result<CallToolResult, ServiceError> {
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(object(args)))
        .await
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect()
}

async fn ok_text(client: &Client, tool: &'static str, args: Value) -> String {
    let result = call(client, tool, args.clone())
        .await
        .unwrap_or_else(|e| panic!("{tool} {args} failed: {e}"));
    assert_ne!(
        result.is_error,
        Some(true),
        "{tool} {args}: {}",
        text_of(&result)
    );
    text_of(&result)
}

async fn error_text(client: &Client, tool: &'static str, args: Value) -> String {
    let result = call(client, tool, args.clone())
        .await
        .unwrap_or_else(|e| panic!("{tool} {args} was a protocol error: {e}"));
    assert_eq!(result.is_error, Some(true), "{tool} {args}");
    text_of(&result)
}

#[tokio::test(flavor = "multi_thread")]
async fn matches_cli_json_for_get_with_and_without_comments() {
    let env = Env::new().await;
    let client = env.connect().await;

    let full = ok_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE}),
    )
    .await;
    assert_eq!(full, env.cli_stdout(&["get", ONE, "--json"]));
    assert!(full.contains("Reproduced on v2.1."), "{full}");

    let bare = ok_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE, "no_comments": true}),
    )
    .await;
    assert_eq!(
        bare,
        env.cli_stdout(&["get", ONE, "--json", "--no-comments"])
    );
    assert!(!bare.contains("Reproduced on v2.1."), "{bare}");

    let refreshed = ok_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE, "refresh": true}),
    )
    .await;
    assert_eq!(refreshed, full);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn matches_cli_json_for_current_mine_and_search() {
    let env = Env::new().await;
    let client = env.connect().await;

    let current = ok_text(
        &client,
        "jira_read",
        json!({"operation": "current", "branch": BRANCH}),
    )
    .await;
    assert_eq!(current, env.cli_stdout(&["current", "--json"]));

    let from_directory = ok_text(&client, "jira_read", json!({"operation": "current"})).await;
    assert_eq!(from_directory, current);

    let mine = ok_text(
        &client,
        "jira_read",
        json!({"operation": "mine", "limit": 5}),
    )
    .await;
    assert_eq!(mine, env.cli_stdout(&["mine", "--json", "--limit", "5"]));
    assert!(mine.contains(ROW_A), "{mine}");

    let jql = "project = PROJ ORDER BY updated DESC";
    let search = ok_text(
        &client,
        "jira_read",
        json!({"operation": "search", "jql": jql}),
    )
    .await;
    assert_eq!(search, env.cli_stdout(&["search", jql, "--json"]));
    assert!(search.contains("Cleanup task"), "{search}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn matches_cli_json_for_search_forwarding_the_limit() {
    let env = Env::new().await;
    let client = env.connect().await;

    ok_text(
        &client,
        "jira_read",
        json!({"operation": "search", "jql": "project = PROJ", "limit": 7}),
    )
    .await;

    let requests = env.server.received_requests().await.unwrap();
    let search = requests
        .iter()
        .find(|request| request.url.path() == "/rest/api/3/search/jql")
        .expect("a search request");
    assert!(
        search
            .url
            .query()
            .unwrap_or_default()
            .contains("maxResults=7"),
        "{}",
        search.url
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn matches_cli_json_for_comment_with_an_explicit_ref() {
    let env = Env::new().await;
    let client = env.connect().await;

    let posted = ok_text(
        &client,
        "jira_write",
        json!({"operation": "comment", "ref": ONE, "body": "Looks good"}),
    )
    .await;
    assert_eq!(
        posted,
        env.cli_stdout(&["comment", ONE, "-m", "Looks good", "--json"])
    );

    let from_branch = ok_text(
        &client,
        "jira_write",
        json!({"operation": "comment", "body": "From the branch"}),
    )
    .await;
    assert_eq!(from_branch, posted);

    let bodies: Vec<String> = env
        .server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "POST")
        .map(|request| String::from_utf8(request.body.clone()).unwrap())
        .collect();
    assert_eq!(bodies.len(), 3);
    assert!(bodies[0].contains("Looks good"), "{}", bodies[0]);
    assert!(bodies[2].contains("From the branch"), "{}", bodies[2]);
    client.cancel().await.unwrap();
}

fn file_contents(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn matches_cli_json_for_download_into_a_directory() {
    let env = Env::new().await;
    let client = env.connect().await;
    let target = env.dir.path().join("saved");
    let target_arg = target.to_str().unwrap();

    let downloaded = ok_text(
        &client,
        "jira_download_attachments",
        json!({"operation": "download", "ref": ONE, "directory": target_arg}),
    )
    .await;

    assert_eq!(file_contents(&target, "screenshot.png"), ATTACHMENT_BODY);
    assert_eq!(file_contents(&target, "notes.txt"), ATTACHMENT_BODY);
    let cli = env.cli_stdout(&[
        "get",
        ONE,
        "--download-attachments",
        "--download-dir",
        target_arg,
        "--json",
    ]);
    assert_eq!(downloaded, cli);
    let parsed: Value = serde_json::from_str(downloaded.trim()).unwrap();
    assert_eq!(parsed["issue_key"], ONE);
    assert_eq!(parsed["saved"].as_array().unwrap().len(), 2);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_from_a_core_are_tool_errors_and_the_server_keeps_serving() {
    let env = Env::new().await;
    let client = env.connect().await;

    for (args, key) in [
        (json!({"operation": "get", "ref": "not a ref"}), "not a ref"),
        (json!({"operation": "get", "ref": MISSING}), MISSING),
        (json!({"operation": "get", "ref": DENIED}), DENIED),
    ] {
        let text = error_text(&client, "jira_read", args.clone()).await;
        let (ok, _, stderr) = env.cli(&["get", key, "--json"]);
        assert!(!ok, "{key} should fail in the CLI too");
        assert_eq!(text, stderr, "{args}");

        let still = ok_text(
            &client,
            "jira_read",
            json!({"operation": "get", "ref": ONE}),
        )
        .await;
        assert!(still.contains(ONE), "{still}");
    }

    let reauth = error_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": DENIED}),
    )
    .await;
    assert!(reauth.contains("jira setup add"), "{reauth}");

    let download = error_text(
        &client,
        "jira_download_attachments",
        json!({"operation": "download", "ref": "not a ref"}),
    )
    .await;
    assert!(download.contains("not a valid issue key"), "{download}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_from_malformed_arguments_are_invalid_params_and_write_nothing() {
    let env = Env::new().await;
    let client = env.connect().await;

    for (tool, args) in [
        ("jira_read", json!({"operation": "frobnicate"})),
        ("jira_write", json!({"operation": "comment", "ref": ONE})),
        (
            "jira_write",
            json!({"operation": "comment", "ref": ONE, "body": "x", "jql": "x"}),
        ),
        ("jira_write", json!({"ref": ONE, "body": "x"})),
        (
            "jira_read",
            json!({"operation": "get", "ref": ONE, "body": "x"}),
        ),
        ("jira_read", json!({"operation": "search"})),
        (
            "jira_download_attachments",
            json!({"operation": "download", "ref": ONE, "body": "x"}),
        ),
    ] {
        match call(&client, tool, args.clone()).await {
            Err(ServiceError::McpError(error)) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{tool} {args}");
            }
            other => panic!("{tool} {args} should be invalid_params, got {other:?}"),
        }
    }
    assert_eq!(env.post_count().await, 0);
    client.cancel().await.unwrap();
}

struct RawSession {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    reader: BufReader<std::process::ChildStdout>,
    lines: Vec<String>,
    next_id: u64,
}

impl RawSession {
    fn start(env: &Env) -> RawSession {
        let mut child = env
            .command()
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let reader = BufReader::new(child.stdout.take().unwrap());
        let mut session = RawSession {
            child,
            stdin,
            reader,
            lines: Vec::new(),
            next_id: 1,
        };
        session.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "raw-test", "version": "0" }
            }),
        );
        session.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        session
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).unwrap();
            assert!(
                read > 0,
                "server closed stdout before answering request {id}"
            );
            let line = line.trim_end().to_owned();
            self.lines.push(line.clone());
            let parsed: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}"));
            if parsed.get("id") == Some(&json!(id)) && parsed.get("method").is_none() {
                return parsed;
            }
        }
    }

    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
    }

    fn finish(mut self) -> Vec<String> {
        drop(self.stdin.take());
        for line in (&mut self.reader).lines() {
            self.lines.push(line.unwrap());
        }
        let status = self.child.wait().unwrap();
        assert!(status.success(), "jira mcp exited with {status}");
        self.lines
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stdout_only_frames_across_every_operation_including_failures() {
    let env = Env::new().await;
    let mut session = RawSession::start(&env);
    let target = env.dir.path().join("frames-saved");

    let calls = [
        ("jira_read", json!({"operation": "get", "ref": ONE})),
        ("jira_read", json!({"operation": "get", "ref": MISSING})),
        ("jira_read", json!({"operation": "get", "ref": DENIED})),
        (
            "jira_read",
            json!({"operation": "current", "branch": BRANCH}),
        ),
        (
            "jira_read",
            json!({"operation": "current", "branch": "main"}),
        ),
        ("jira_read", json!({"operation": "mine"})),
        (
            "jira_read",
            json!({"operation": "search", "jql": "project = PROJ"}),
        ),
        ("jira_read", json!({"operation": "search", "jql": "  "})),
        (
            "jira_write",
            json!({"operation": "comment", "ref": ONE, "body": "hi"}),
        ),
        (
            "jira_write",
            json!({"operation": "comment", "ref": ONE, "body": ""}),
        ),
        (
            "jira_download_attachments",
            json!({"operation": "download", "ref": ONE, "directory": target}),
        ),
        ("jira_read", json!({"operation": "frobnicate"})),
    ];
    for (tool, args) in &calls {
        let reply = session.tool(tool, args.clone());
        assert!(
            reply.get("result").is_some() || reply.get("error").is_some(),
            "{tool} {args}: {reply}"
        );
    }

    let lines = session.finish();
    assert!(lines.len() > calls.len());
    for line in &lines {
        let frame: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}"));
        assert_eq!(frame["jsonrpc"], "2.0", "{line}");
    }
}

fn journal_mode(db: &Path) -> String {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn two_servers_share_one_wal_database_without_lock_errors() {
    let env = Env::new().await;
    let first = env.connect().await;
    let second = env.connect().await;

    for round in 0..8 {
        let key = if round % 2 == 0 { ONE } else { TWO };
        let args = json!({"operation": "get", "ref": key, "refresh": true});
        let (a, b) = tokio::join!(
            call(&first, "jira_read", args.clone()),
            call(&second, "jira_read", args.clone())
        );
        for result in [a.unwrap(), b.unwrap()] {
            let text = text_of(&result);
            assert_ne!(result.is_error, Some(true), "{text}");
            assert!(!text.contains("database is locked"), "{text}");
        }
    }

    assert_eq!(journal_mode(&env.db), "wal");
    first.cancel().await.unwrap();
    second.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_ref_overrides_the_branch_issue_when_commenting() {
    let env = Env::new().await;
    let client = env.connect().await;

    ok_text(
        &client,
        "jira_write",
        json!({"operation": "comment", "ref": TWO, "body": "For the other issue"}),
    )
    .await;

    let posts: Vec<String> = env
        .server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "POST")
        .map(|request| request.url.path().to_owned())
        .collect();
    assert!(
        posts.contains(&format!("/rest/api/3/issue/{TWO}/comment")),
        "{posts:?}"
    );
    assert!(
        !posts.contains(&format!("/rest/api/3/issue/{ONE}/comment")),
        "{posts:?}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_branch_overrides_the_checked_out_branch_for_current() {
    let env = Env::new().await;
    let client = env.connect().await;

    let current = ok_text(
        &client,
        "jira_read",
        json!({"operation": "current", "branch": format!("feature/{TWO}-signup")}),
    )
    .await;

    let value: Value = serde_json::from_str(&current).unwrap();
    assert_eq!(value["key"], TWO, "{current}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_instance_routes_the_request_to_that_instance() {
    let env = Env::new().await;
    let other = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{ONE}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_payload(ONE, &other.uri())))
        .mount(&other)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "acc-me",
            "displayName": "Me",
            "active": true,
            "self": "",
            "avatarUrls": {}
        })))
        .mount(&other)
        .await;
    env.add_instance("other", &other.uri());
    let client = env.connect().await;

    ok_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE, "instance": "other"}),
    )
    .await;

    let hits = |requests: Vec<wiremock::Request>| {
        requests
            .iter()
            .filter(|request| request.url.path() == format!("/rest/api/3/issue/{ONE}"))
            .count()
    };
    assert_eq!(hits(other.received_requests().await.unwrap()), 1);
    assert_eq!(hits(env.server.received_requests().await.unwrap()), 0);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn initialize_reports_server_info_named_jira_at_the_crate_version() {
    let env = Env::new().await;
    let client = env.connect().await;

    let info = serde_json::to_value(&*client.peer_info().expect("handshake info")).unwrap();
    assert_eq!(info["serverInfo"]["name"], "jira", "{info}");
    assert_eq!(
        info["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION"),
        "{info}"
    );
    client.cancel().await.unwrap();
}

const SKILL_URI: &str = "skill://jira-ticket/SKILL.md";

#[tokio::test(flavor = "multi_thread")]
async fn initialize_declares_the_skills_extension_and_points_the_instructions_at_the_skill() {
    let env = Env::new().await;
    let client = env.connect().await;

    let info = serde_json::to_value(&*client.peer_info().expect("handshake info")).unwrap();
    assert!(info["capabilities"]["resources"].is_object(), "{info}");
    assert!(
        info["capabilities"]["extensions"]
            .as_object()
            .is_some_and(|extensions| extensions.keys().any(|key| key.contains("skills"))),
        "{info}"
    );
    assert_eq!(info["serverInfo"]["name"], "jira", "{info}");
    assert!(
        info["instructions"]
            .as_str()
            .is_some_and(|text| text.contains(SKILL_URI)),
        "{info}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_skill_is_served_as_a_resource_and_as_a_catalog_entry_equal_to_the_cli_text() {
    let env = Env::new().await;
    let client = env.connect().await;
    let printed = env.cli_stdout(&["skill", "jira-ticket"]);

    let resources = client.list_all_resources().await.unwrap();
    assert_eq!(resources.len(), 1, "{resources:?}");
    assert_eq!(resources[0].uri, SKILL_URI);
    assert_eq!(resources[0].mime_type.as_deref(), Some("text/markdown"));
    assert_eq!(resources[0].name, "jira-ticket");

    let read = client
        .read_resource(rmcp::model::ReadResourceRequestParams::new(SKILL_URI))
        .await
        .unwrap();
    let read = serde_json::to_value(&read).unwrap();
    assert_eq!(read["contents"][0]["text"], printed.as_str(), "{read}");
    assert_eq!(read["contents"][0]["mimeType"], "text/markdown", "{read}");

    let skills = client.list_all_skills().await.unwrap();
    assert_eq!(skills.len(), 1, "{skills:?}");
    let listed = serde_json::to_value(&skills[0]).unwrap();
    assert_eq!(listed["uri"], SKILL_URI, "{listed}");
    let description = printed
        .lines()
        .find_map(|line| line.strip_prefix("description: "))
        .expect("the skill text has a description");
    assert_eq!(listed["frontmatter"]["name"], "jira-ticket", "{listed}");
    assert_eq!(
        listed["frontmatter"]["description"], description,
        "{listed}"
    );

    let got = client
        .get_skill(rmcp::model::GetSkillRequestParams::new(SKILL_URI))
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(&got.skill).unwrap(), listed);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_resources_and_skills_are_invalid_params_and_the_server_keeps_serving() {
    let env = Env::new().await;
    let mut session = RawSession::start(&env);

    for (method, params) in [
        (
            "resources/read",
            json!({"uri": "skill://jira-ticket/other.md"}),
        ),
        (
            "resources/read",
            json!({"uri": "skill://jira-ticket/../SKILL.md"}),
        ),
        ("resources/read", json!({"uri": "file:///etc/passwd"})),
        ("skills/get", json!({"uri": "skill://jira-ticket/other.md"})),
        ("skills/get", json!({})),
    ] {
        let reply = session.request(method, params.clone());
        assert_eq!(
            reply["error"]["code"],
            json!(ErrorCode::INVALID_PARAMS.0),
            "{method} {params}: {reply}"
        );
    }
    let still = session.request("resources/read", json!({"uri": SKILL_URI}));
    assert!(
        still["result"]["contents"][0]["text"].is_string(),
        "{still}"
    );
    session.finish();
}

async fn call_once(peer: &Peer<RoleClient>, tool: &'static str, args: Value) -> CallToolResponse {
    let params = CallToolRequestParams::new(tool).with_arguments(object(args));
    peer.call_tool_once(params).await.unwrap()
}

async fn task_id_of(peer: &Peer<RoleClient>, tool: &'static str, args: Value) -> String {
    match call_once(peer, tool, args).await {
        CallToolResponse::Task(created) => created.task.task_id,
        other => panic!("{tool} was not answered with a task: {other:?}"),
    }
}

async fn complete_text(peer: &Peer<RoleClient>, tool: &'static str, args: Value) -> (bool, String) {
    match call_once(peer, tool, args).await {
        CallToolResponse::Complete(result) => (result.is_error == Some(true), text_of(&result)),
        other => panic!("{tool} was not answered directly: {other:?}"),
    }
}

async fn task_state(peer: &Peer<RoleClient>, id: &str) -> Value {
    let got = peer.get_task(GetTaskParams::new(id)).await.unwrap();
    serde_json::to_value(&got).unwrap()
}

/// Polls `tasks/get` until the task leaves `working`, or panics after `within`.
async fn settled_state(peer: &Peer<RoleClient>, id: &str, within: Duration) -> Value {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let state = task_state(peer, id).await;
        if state["status"] != "working" {
            return state;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "task still working after {within:?}: {state}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn result_text(state: &Value) -> String {
    state["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no result text in {state}"))
        .to_owned()
}

const SEARCH_JQL: &str = "project = PROJ";

#[tokio::test(flavor = "multi_thread")]
async fn initialize_declares_the_tasks_extension() {
    let env = Env::new().await;
    let client = env.connect().await;

    let info = serde_json::to_value(&*client.peer_info().expect("handshake info")).unwrap();
    assert!(
        info["capabilities"]["extensions"]
            .as_object()
            .is_some_and(|extensions| extensions.keys().any(|key| key.contains("tasks"))),
        "{info}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_task_completes_with_the_synchronous_result() {
    let env = Env::new().await;
    let target = env.dir.path().join("saved");
    let args = json!({"operation": "download", "ref": ONE, "directory": target.to_str().unwrap()});
    let plain = env.connect().await;
    let synchronous = ok_text(&plain, "jira_download_attachments", args.clone()).await;
    plain.cancel().await.unwrap();

    let (client, _progress) = env.connect_declaring_tasks().await;
    let id = task_id_of(&client, "jira_download_attachments", args).await;
    let state = settled_state(&client, &id, Duration::from_secs(10)).await;

    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(result_text(&state), synchronous);
    assert_eq!(
        synchronous,
        env.cli_stdout(&[
            "get",
            ONE,
            "--download-attachments",
            "--download-dir",
            target.to_str().unwrap(),
            "--json",
        ])
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_above_the_default_limit_is_a_task_and_one_at_it_is_direct() {
    let env = Env::new().await;
    let (client, _progress) = env.connect_declaring_tasks().await;

    let id = task_id_of(
        &client,
        "jira_read",
        json!({"operation": "search", "jql": SEARCH_JQL, "limit": 51}),
    )
    .await;
    let state = settled_state(&client, &id, Duration::from_secs(10)).await;
    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(
        result_text(&state),
        env.cli_stdout(&["search", SEARCH_JQL, "--json"])
    );

    let (failed, at_default) = complete_text(
        &client,
        "jira_read",
        json!({"operation": "search", "jql": SEARCH_JQL, "limit": 50}),
    )
    .await;
    assert!(!failed, "{at_default}");
    assert_eq!(
        at_default,
        env.cli_stdout(&["search", SEARCH_JQL, "--json"])
    );
    let (failed, issue) = complete_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE}),
    )
    .await;
    assert!(!failed, "{issue}");
    assert_eq!(issue, env.cli_stdout(&["get", ONE, "--json"]));
    let (failed, posted) = complete_text(
        &client,
        "jira_write",
        json!({"operation": "comment", "ref": ONE, "body": "direct"}),
    )
    .await;
    assert!(!failed, "{posted}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_download_task_ends_it_cancelled_and_the_server_keeps_serving() {
    let env = Env::new().await;
    env.delay_attachments(Duration::from_secs(30)).await;
    let (client, _progress) = env.connect_declaring_tasks().await;
    let target = env.dir.path().join("slow");

    let id = task_id_of(
        &client,
        "jira_download_attachments",
        json!({"operation": "download", "ref": ONE, "directory": target.to_str().unwrap()}),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    client
        .cancel_task(CancelTaskParams::new(&id))
        .await
        .unwrap();

    let state = settled_state(&client, &id, Duration::from_secs(5)).await;
    assert_eq!(state["status"], "cancelled", "{state}");
    let (failed, issue) = complete_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE}),
    )
    .await;
    assert!(!failed, "{issue}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_task_whose_core_fails_completes_with_the_error_text_of_the_cli() {
    let env = Env::new().await;
    let (client, _progress) = env.connect_declaring_tasks().await;

    let id = task_id_of(
        &client,
        "jira_download_attachments",
        json!({"operation": "download", "ref": MISSING}),
    )
    .await;
    let state = settled_state(&client, &id, Duration::from_secs(10)).await;

    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(state["result"]["isError"], true, "{state}");
    let (ok, _, stderr) = env.cli(&["get", MISSING, "--download-attachments", "--json"]);
    assert!(!ok);
    assert_eq!(result_text(&state), stderr);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_task_call_reports_progress_for_its_token_and_a_get_reports_none() {
    let env = Env::new().await;
    env.delay_attachments(Duration::from_secs(6)).await;
    let (client, mut progress) = env.connect_declaring_tasks().await;
    let target = env.dir.path().join("beat");

    let (failed, issue) = complete_text(
        &client,
        "jira_read",
        json!({"operation": "get", "ref": ONE}),
    )
    .await;
    assert!(!failed, "{issue}");
    assert!(progress.try_recv().is_err(), "a get reported progress");

    let id = task_id_of(
        &client,
        "jira_download_attachments",
        json!({"operation": "download", "ref": ONE, "directory": target.to_str().unwrap()}),
    )
    .await;
    let state = settled_state(&client, &id, Duration::from_secs(20)).await;

    assert_eq!(state["status"], "completed", "{state}");
    assert!(
        progress.try_recv().is_ok(),
        "no progress notification while the task ran"
    );
    client.cancel().await.unwrap();
}
