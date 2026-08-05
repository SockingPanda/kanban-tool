use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{CreateStepRequest, CreateStepResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    pub(crate) task_ref: String,
    pub(crate) title: String,
    #[arg(long)]
    pub(crate) body: Option<String>,
    #[arg(long = "link-task")]
    pub(crate) linked_task_ref: Option<String>,
    #[arg(long)]
    pub(crate) position: Option<i64>,
    #[arg(long, conflicts_with = "optional")]
    pub(crate) required: bool,
    #[arg(long, conflicts_with = "required")]
    pub(crate) optional: bool,
    #[arg(long)]
    pub(crate) idempotency_key: Option<String>,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &AddArgs,
) -> Result<(), CliFailure> {
    let steps = client.create_step_by_selector(
        &ctx.board,
        &args.task_ref,
        &CreateStepRequest {
            idempotency_key: args.idempotency_key.clone(),
            title: args.title.clone(),
            body: args.body.clone(),
            linked_task_ref: args.linked_task_ref.clone(),
            position: args.position,
            required: !args.optional,
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CreateStepResponse { data: steps });
    } else {
        for (index, step) in steps.steps.iter().enumerate() {
            println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
        }
    }
    Ok(())
}
