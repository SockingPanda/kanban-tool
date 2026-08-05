use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{ReleaseTaskRequest, ReleaseTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ReleaseArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ReleaseArgs,
) -> Result<(), CliFailure> {
    let task = client.release_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &ReleaseTaskRequest {
            actor: None,
            claim_token: args.claim_token.clone(),
        },
    )?;
    if ctx.json {
        output::print_json(&ReleaseTaskResponse::new(task));
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
    fn parses_task_release_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "release",
            "default#1",
            "--claim-token",
            "claim_test",
        ])
        .expect("release args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Release(args),
        } = cli.command
        else {
            panic!("expected task release");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token, "claim_test");
    }
}
