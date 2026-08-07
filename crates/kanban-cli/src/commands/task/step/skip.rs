use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{CliTaskStepSkipOutput, SkipStepRequest};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct SkipArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) reason: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &SkipArgs,
) -> Result<(), CliFailure> {
    let step = client.skip_step_by_selector(
        &ctx.board,
        &args.task_ref,
        &args.step_ref,
        &SkipStepRequest {
            reason: args.reason.clone(),
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CliTaskStepSkipOutput::new(step));
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
