use crate::{context::CliContext, error::CliFailure, output};
use clap::{Args, ValueEnum};
use kanban_client::KanbanClient;
use kanban_contract::{ReclaimTargetStatus, ReclaimTaskRequest, ReclaimTaskResponse};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ReclaimStatus {
    Ready,
    Blocked,
}
#[derive(Debug, Args)]
pub(crate) struct ReclaimArgs {
    #[arg(help = "全局任务 ID 或看板内引用")]
    pub(crate) task_ref: String,
    #[arg(long, help = "即使 claim 未过期也强制回收")]
    pub(crate) force: bool,
    #[arg(long, value_enum, help = "回收后的目标状态；缺省时按当前事实重算")]
    pub(crate) to_status: Option<ReclaimStatus>,
    #[arg(long, help = "回收原因")]
    pub(crate) reason: Option<String>,
}
pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ReclaimArgs,
) -> Result<(), CliFailure> {
    let to_status = args.to_status.map(|status| match status {
        ReclaimStatus::Ready => ReclaimTargetStatus::Ready,
        ReclaimStatus::Blocked => ReclaimTargetStatus::Blocked,
    });
    let task = client.reclaim_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &ReclaimTaskRequest {
            actor: None,
            force: args.force,
            to_status,
            reason: args.reason.clone(),
        },
    )?;
    if ctx.json {
        output::print_json(&ReclaimTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
