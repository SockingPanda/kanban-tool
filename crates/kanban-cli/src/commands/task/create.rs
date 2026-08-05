use std::collections::BTreeMap;

use clap::{Args, ValueEnum};
use kanban_client::KanbanClient;
use kanban_contract::{ApiCreateTaskStatus, CreateTaskRequest, CreateTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct CreateArgs {
    pub(crate) title: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) status: Option<CreateStatus>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long, default_value_t = 3)]
    pub(crate) priority: i64,
    #[arg(long)]
    pub(crate) scheduled_at: Option<i64>,
    #[arg(long)]
    pub(crate) due_at: Option<i64>,
    #[arg(long)]
    pub(crate) max_retries: Option<i64>,
    /// JSON object stored as task metadata.
    #[arg(long)]
    pub(crate) metadata: Option<String>,
    /// Stable retry key scoped to this board.
    #[arg(long)]
    pub(crate) idempotency_key: Option<String>,
    /// Optional client-selected typed task id.
    #[arg(long)]
    pub(crate) task_id: Option<String>,
    /// Attach existing board labels by name or id (repeatable).
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
    /// Add existing parent tasks by global id (repeatable).
    #[arg(long = "depends-on")]
    pub(crate) depends_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CreateStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &CreateArgs,
) -> Result<(), CliFailure> {
    let metadata = parse_metadata(args.metadata.as_deref())?;
    let task = client.create_task(
        &ctx.board,
        CreateTaskRequest {
            task_id: args.task_id.clone(),
            idempotency_key: args.idempotency_key.clone(),
            title: args.title.clone(),
            description: args.description.clone(),
            status: args.status.map(api_create_status),
            assignee: args.assignee.clone(),
            priority: args.priority,
            scheduled_at: args.scheduled_at,
            due_at: args.due_at,
            max_retries: args.max_retries,
            metadata,
            labels: args.labels.clone(),
            depends_on: args.depends_on.clone(),
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CreateTaskResponse { data: task });
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}

fn api_create_status(status: CreateStatus) -> ApiCreateTaskStatus {
    match status {
        CreateStatus::Triage => ApiCreateTaskStatus::Triage,
        CreateStatus::Todo => ApiCreateTaskStatus::Todo,
        CreateStatus::Scheduled => ApiCreateTaskStatus::Scheduled,
        CreateStatus::Ready => ApiCreateTaskStatus::Ready,
    }
}

fn parse_metadata(
    metadata: Option<&str>,
) -> Result<Option<BTreeMap<String, serde_json::Value>>, CliFailure> {
    metadata
        .map(|metadata| {
            serde_json::from_str(metadata).map_err(|error| CliFailure {
                code: "invalid_input",
                message: format!("--metadata must be a JSON object: {error}"),
                exit_code: 2,
            })
        })
        .transpose()
}
