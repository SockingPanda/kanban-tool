use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{GetTaskDetailsResponse, GetTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) task_ref: String,
    /// Include canonical labels, dependencies, plan, steps, comments, runs and events.
    #[arg(long)]
    pub(crate) details: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ShowArgs,
) -> Result<(), CliFailure> {
    if args.details {
        let detail = client.get_task_details_by_selector(&ctx.board, &args.task_ref)?;
        if ctx.json {
            output::print_json(&GetTaskDetailsResponse { data: detail });
        } else {
            println!(
                "{} {} {} (labels: {}, dependencies: {}, steps: {}, comments: {}, runs: {}, events: {})",
                detail.task.task_ref,
                detail.task.status.as_str(),
                detail.task.title,
                detail.labels.len(),
                detail.dependencies.parents.len(),
                detail.steps.len(),
                detail.comments.len(),
                detail.runs.len(),
                detail.events.len(),
            );
        }
        return Ok(());
    }
    let task = client.get_task_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&GetTaskResponse::new(task, None));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
