use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::GetTaskResponse;

use crate::{
    context::CliContext,
    error::{CliFailure, feature_not_available},
    output,
};

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) task_ref: String,
    /// Ontology details are intentionally unavailable on the single-host path.
    #[arg(long)]
    pub(crate) details: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ShowArgs,
) -> Result<(), CliFailure> {
    if args.details {
        return Err(feature_not_available(
            "`task show --details` requires the deferred ontology projection",
        ));
    }
    let task = client.get_task_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&GetTaskResponse::new(task, None));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}
