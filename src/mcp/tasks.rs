//! Which tool calls run as MCP tasks, and the runner that drives one: a heartbeat of progress
//! notifications while the call runs, and a stop when the call is cancelled.

use super::{run_isolated, CoreRun, ReadOperation};
use crate::commands::DEFAULT_SEARCH_LIMIT;
use rmcp::model::{
    CallToolResponse, CallToolResult, ContentBlock, CreateTaskResult, ProgressNotificationParam,
    ProgressToken,
};
use rmcp::service::RequestContext;
use rmcp::task_manager::{TaskExit, TaskManager, TaskOptions};
use rmcp::{ErrorData as McpError, Peer, RoleServer};
use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;

/// How often a running task-eligible call tells the client it is alive.
const HEARTBEAT: Duration = Duration::from_secs(5);

/// The poll interval a task advertises to its client.
const POLL_INTERVAL_MS: u64 = 1_000;

/// A tool call that may be long-running, as far as the task decision needs to see it. A comment
/// is never one, so it has no variant.
pub(super) enum Call<'a> {
    Read(&'a ReadOperation),
    Download,
}

/// Whether the call is answered with a task. Only a client that declared the tasks extension
/// ever gets one; of its calls, a download and a search for more than the default limit can run
/// long enough to need it.
pub(super) fn runs_as_task(declared: bool, call: &Call<'_>) -> bool {
    declared && is_long_running(call)
}

/// Whether the call can outlast a client's patience for a plain request, so it reports
/// progress while it runs and may run as a task.
pub(super) fn is_long_running(call: &Call<'_>) -> bool {
    match call {
        Call::Download => true,
        Call::Read(ReadOperation::Search { limit, .. }) => {
            limit.is_some_and(|limit| limit > DEFAULT_SEARCH_LIMIT)
        }
        Call::Read(
            ReadOperation::Get { .. } | ReadOperation::Current { .. } | ReadOperation::Mine { .. },
        ) => false,
    }
}

/// Where a long-running call reports progress: the client's progress token, when it sent one.
#[derive(Clone)]
pub(super) struct Relay {
    peer: Peer<RoleServer>,
    token: Option<ProgressToken>,
}

impl Relay {
    pub(super) fn of(context: &RequestContext<RoleServer>) -> Relay {
        Relay {
            peer: context.peer.clone(),
            token: context.meta.get_progress_token(),
        }
    }

    async fn beat(&self, elapsed: Duration) {
        let Some(token) = &self.token else { return };
        let notification = ProgressNotificationParam::new(token.clone(), elapsed.as_secs_f64())
            .with_message("still running");
        let _ = self.peer.notify_progress(notification).await;
    }
}

/// Awaits `work`, sending a progress notification every `HEARTBEAT` while it runs.
async fn with_heartbeat<T>(work: impl Future<Output = T>, relay: &Relay) -> T {
    if relay.token.is_none() {
        return work.await;
    }
    tokio::pin!(work);
    let started = Instant::now();
    let mut ticker = tokio::time::interval_at(started + HEARTBEAT, HEARTBEAT);
    loop {
        tokio::select! {
            output = &mut work => return output,
            _ = ticker.tick() => relay.beat(started.elapsed()).await,
        }
    }
}

/// Runs a call that is never a task: no heartbeat and no cancellation.
pub(super) async fn run_plain<F, Fut>(make: F) -> Result<CallToolResponse, McpError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = CoreRun>,
{
    let run = run_isolated(make, std::future::pending()).await?;
    Ok(response_of(run))
}

/// Runs a long-running call. For a task-declaring client the call becomes a task that
/// `tasks/get` completes and `tasks/cancel` ends; otherwise it is answered directly, still
/// reporting progress and stopping when the request is cancelled.
pub(super) async fn run_long<F, Fut>(
    tasks: &TaskManager,
    context: RequestContext<RoleServer>,
    as_task: bool,
    make: F,
) -> Result<CallToolResponse, McpError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = CoreRun>,
{
    let relay = Relay::of(&context);
    if as_task {
        let options = TaskOptions::new()
            .with_poll_interval_ms(POLL_INTERVAL_MS)
            .with_ttl_ms(None);
        let task = tasks.spawn(options, move |task| {
            Box::pin(async move {
                let stop = {
                    let task = task.clone();
                    async move { task.cancelled().await }
                };
                match with_heartbeat(run_isolated(make, stop), &relay).await {
                    Ok(Some(run)) => Ok(run.into_result()),
                    Ok(None) => Err(TaskExit::Cancelled),
                    Err(error) => Err(TaskExit::Error(error)),
                }
            })
        });
        return Ok(CallToolResponse::Task(CreateTaskResult::new(task)));
    }
    let stop = {
        let token = context.ct.clone();
        async move { token.cancelled().await }
    };
    let run = with_heartbeat(run_isolated(make, stop), &relay).await?;
    Ok(response_of(run))
}

fn response_of(run: Option<CoreRun>) -> CallToolResponse {
    let result = match run {
        Some(run) => run.into_result(),
        None => CallToolResult::error(vec![ContentBlock::text("cancelled")]),
    };
    CallToolResponse::Complete(result)
}
