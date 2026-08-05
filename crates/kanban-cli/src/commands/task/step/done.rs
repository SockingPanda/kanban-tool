use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{CliTaskStepDoneOutput, CompleteStepRequest};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct DoneArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) note: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &DoneArgs,
) -> Result<(), CliFailure> {
    let step = client.complete_step_by_selector(
        &ctx.board,
        &args.task_ref,
        &args.step_ref,
        &CompleteStepRequest {
            note: args.note.clone(),
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CliTaskStepDoneOutput::new(step));
    } else {
        println!(
            "step {}：{} {}",
            args.step_ref,
            step.status.as_str(),
            step.title
        );
    }
    Ok(())
}
