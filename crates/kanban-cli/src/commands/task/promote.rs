use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{PromoteTaskRequest, PromoteTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct PromoteArgs {
    pub(crate) task_ref: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &PromoteArgs,
) -> Result<(), CliFailure> {
    let task = client.promote_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &PromoteTaskRequest { actor: None },
    )?;
    if ctx.json {
        output::print_json(&PromoteTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_task_promote_command() {
        let cli =
            Cli::try_parse_from(["kanban", "task", "promote", "default#1"]).expect("promote args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Promote(args),
        } = cli.command
        else {
            panic!("expected task promote");
        };
        assert_eq!(args.task_ref, "default#1");
    }
}
