use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{ReopenTaskRequest, ReopenTaskResponse};

#[derive(Debug, Args)]
pub(crate) struct ReopenArgs {
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
    #[arg(long, help = "重新打开原因")]
    pub(crate) reason: String,
}
pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ReopenArgs,
) -> Result<(), CliFailure> {
    let task = client.reopen_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &ReopenTaskRequest {
            actor: None,
            reason: args.reason.clone(),
        },
    )?;
    if ctx.json {
        output::print_json(&ReopenTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
