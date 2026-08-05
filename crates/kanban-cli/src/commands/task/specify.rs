use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{SpecifyTaskRequest, SpecifyTaskResponse};

#[derive(Debug, Args)]
pub(crate) struct SpecifyArgs {
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
    #[arg(long, help = "补充任务描述")]
    pub(crate) description: Option<String>,
    #[arg(long, help = "设置计划开始时间（毫秒时间戳）")]
    pub(crate) scheduled_at: Option<i64>,
}
pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &SpecifyArgs,
) -> Result<(), CliFailure> {
    let task = client.specify_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &SpecifyTaskRequest {
            actor: None,
            description: args.description.clone(),
            scheduled_at: args.scheduled_at,
        },
    )?;
    if ctx.json {
        output::print_json(&SpecifyTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
