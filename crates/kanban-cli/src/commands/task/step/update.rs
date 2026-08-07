use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{UpdateStepRequest, UpdateStepResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) body: Option<String>,
    #[arg(long = "link-task", conflicts_with = "unlink_task")]
    pub(crate) linked_task_ref: Option<String>,
    #[arg(long, conflicts_with = "linked_task_ref")]
    pub(crate) unlink_task: bool,
    #[arg(long)]
    pub(crate) position: Option<i64>,
    #[arg(long, conflicts_with = "optional")]
    pub(crate) required: bool,
    #[arg(long, conflicts_with = "required")]
    pub(crate) optional: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &UpdateArgs,
) -> Result<(), CliFailure> {
    let steps = client.update_step_by_selector(
        &ctx.board,
        &args.task_ref,
        &args.step_ref,
        &UpdateStepRequest {
            title: args.title.clone(),
            body: args.body.clone(),
            linked_task_ref: args.linked_task_ref.clone(),
            unlink_task: args.unlink_task,
            position: args.position,
            required: if args.required {
                Some(true)
            } else if args.optional {
                Some(false)
            } else {
                None
            },
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&UpdateStepResponse { data: steps });
    } else {
        for (index, step) in steps.steps.iter().enumerate() {
            println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_task_step_update_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "step",
            "update",
            "default#1",
            "S2",
            "--title",
            "Updated",
            "--position",
            "2048",
            "--optional",
        ])
        .expect("step update args");
        let Command::Task {
            command:
                crate::commands::task::TaskCommand::Step {
                    command: crate::commands::task::step::TaskStepCommand::Update(args),
                },
        } = cli.command
        else {
            panic!("expected task step update");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.step_ref, "S2");
        assert_eq!(args.title.as_deref(), Some("Updated"));
        assert_eq!(args.position, Some(2048));
        assert!(args.optional);
    }
}
