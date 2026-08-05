use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{ArchiveTaskRequest, ArchiveTaskResponse};

#[derive(Debug, Args)]
pub(crate) struct ArchiveArgs {
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
    #[arg(long, help = "强制归档 running 任务或未完成必需步骤")]
    pub(crate) force: bool,
}
pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ArchiveArgs,
) -> Result<(), CliFailure> {
    let task = client.archive_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &ArchiveTaskRequest {
            actor: None,
            force: args.force,
        },
    )?;
    if ctx.json {
        output::print_json(&ArchiveTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
