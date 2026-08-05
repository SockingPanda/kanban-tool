use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct PlanNotRequiredArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) reason: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &PlanNotRequiredArgs,
) -> Result<(), CliFailure> {
    let plan = client.mark_execution_plan_not_required_by_selector(
        &ctx.board,
        &args.task_ref,
        &MarkExecutionPlanNotRequiredRequest {
            reason: args.reason.clone(),
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&MarkExecutionPlanNotRequiredResponse { data: plan });
    } else {
        println!(
            "{} {} {}",
            plan.task_id,
            plan.state.as_str(),
            plan.reason.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_execution_plan_not_required_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "step",
            "not-required",
            "default#1",
            "--reason",
            "small task",
        ])
        .unwrap();
        let Command::Task {
            command:
                crate::commands::task::TaskCommand::Step {
                    command: crate::commands::task::step::TaskStepCommand::NotRequired(args),
                },
        } = cli.command
        else {
            panic!("expected task step not-required");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.reason, "small task");
    }
}
