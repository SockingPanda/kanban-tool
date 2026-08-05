use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{UnblockTaskRequest, UnblockTaskResponse};

#[derive(Debug, Args)]
pub(crate) struct UnblockArgs {
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
}
pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &UnblockArgs,
) -> Result<(), CliFailure> {
    let task = client.unblock_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &UnblockTaskRequest { actor: None },
    )?;
    if ctx.json {
        output::print_json(&UnblockTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
