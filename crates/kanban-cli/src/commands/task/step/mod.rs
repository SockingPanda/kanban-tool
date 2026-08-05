pub(super) mod add;
pub(super) mod list;
pub(super) mod update;

use clap::Subcommand;
use kanban_client::KanbanClient;

use crate::{context::CliContext, error::CliFailure};

use super::plan_not_required;

#[derive(Debug, Subcommand)]
pub(crate) enum TaskStepCommand {
    /// Add a todo step to a task execution plan.
    Add(add::AddArgs),
    /// List the task execution plan steps.
    List(list::ListArgs),
    /// Update editable execution-plan fields without changing step status.
    Update(update::UpdateArgs),
    /// Mark this task as not requiring structured execution steps.
    NotRequired(plan_not_required::PlanNotRequiredArgs),
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    command: &TaskStepCommand,
) -> Result<(), CliFailure> {
    match command {
        TaskStepCommand::Add(args) => add::run(ctx, client, args),
        TaskStepCommand::List(args) => list::run(ctx, client, args),
        TaskStepCommand::Update(args) => update::run(ctx, client, args),
        TaskStepCommand::NotRequired(args) => plan_not_required::run(ctx, client, args),
    }
}
