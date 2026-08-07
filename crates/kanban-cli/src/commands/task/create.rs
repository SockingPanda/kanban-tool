use std::collections::BTreeMap;

use clap::{Args, ValueEnum};
use kanban_client::KanbanClient;
use kanban_protocol::{ApiCreateTaskStatus, CreateTaskRequest, CreateTaskResponse};

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
    /// 作为任务 metadata 存储的 JSON 对象。
    #[arg(long)]
    pub(crate) metadata: Option<String>,
    /// 作用域限定在此看板内的稳定重试 key。
    #[arg(long)]
    pub(crate) idempotency_key: Option<String>,
    /// 可选的、由 client 选择的 typed task ID。
    #[arg(long)]
    pub(crate) task_id: Option<String>,
    /// 按名称或 ID 绑定已有看板 label（可重复）。
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
    /// 按全局 ID 添加已有父任务（可重复）。
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
                message: format!("--metadata 必须是 JSON 对象：{error}"),
                exit_code: 2,
            })
        })
        .transpose()
}
