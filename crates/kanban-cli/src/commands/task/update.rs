use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{UpdateTaskRequest, UpdateTaskResponse};

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    #[arg(long = "module", conflicts_with = "clear_modules")]
    pub(crate) module_ids: Vec<String>,
    #[arg(long = "cycle", conflicts_with = "clear_cycle")]
    pub(crate) cycle_id: Option<String>,
    #[arg(long)]
    pub(crate) clear_modules: bool,
    #[arg(long)]
    pub(crate) clear_cycle: bool,
    #[arg(long)]
    pub(crate) request_id: Option<String>,
    #[arg(long)]
    pub(crate) expected_object_version: Option<i64>,
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
    #[arg(long, help = "更新任务标题")]
    pub(crate) title: Option<String>,
    #[arg(long, help = "更新描述；传入空值可清除")]
    pub(crate) description: Option<String>,
    #[arg(long, help = "更新负责人；传入空值可清除")]
    pub(crate) assignee: Option<String>,
    #[arg(long, help = "更新优先级，范围为 0 到 3")]
    pub(crate) priority: Option<i64>,
    #[arg(long, help = "更新计划开始时间（毫秒时间戳）")]
    pub(crate) scheduled_at: Option<i64>,
    #[arg(long, help = "更新截止时间（毫秒时间戳）")]
    pub(crate) due_at: Option<i64>,
    #[arg(long, help = "更新最大重试次数，必须大于 0")]
    pub(crate) max_retries: Option<i64>,
    #[arg(long, value_name = "JSON", help = "更新 metadata JSON")]
    pub(crate) metadata: Option<String>,
    #[arg(long, help = "乐观锁版本；缺省时读取当前版本")]
    pub(crate) expected_lock_version: Option<i64>,
}

pub(crate) async fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &UpdateArgs,
) -> Result<(), CliFailure> {
    let metadata = args
        .metadata
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| CliFailure {
            code: "invalid_input",
            message: format!("metadata JSON 无效: {error}"),
            exit_code: 2,
        })?;
    let task = client
        .update_task_by_selector(
            &ctx.board,
            &args.task_ref,
            &UpdateTaskRequest {
                module_ids: if args.clear_modules || !args.module_ids.is_empty() {
                    Some(args.module_ids.clone())
                } else {
                    None
                },
                cycle_id: if args.clear_cycle {
                    Some(None)
                } else {
                    args.cycle_id.clone().map(Some)
                },
                expected_object_version: args.expected_object_version,
                expected_versions: Default::default(),
                expected_catalog_version: None,
                request_id: args.request_id.clone(),
                title: args.title.clone(),
                description: args.description.clone().map(Some),
                assignee: args.assignee.clone().map(Some),
                priority: args.priority,
                scheduled_at: args.scheduled_at.map(Some),
                due_at: args.due_at.map(Some),
                max_retries: args.max_retries.map(Some),
                metadata: metadata.map(Some),
                actor: None,
                expected_lock_version: args.expected_lock_version,
            },
        )
        .await?;
    if ctx.json {
        output::print_json(&UpdateTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
