use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::ListStepsResponse;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    pub(crate) task_ref: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ListArgs,
) -> Result<(), CliFailure> {
    let steps = client.list_steps_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&ListStepsResponse { data: steps });
    } else {
        for (index, step) in steps.steps.iter().enumerate() {
            println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
        }
    }
    Ok(())
}
