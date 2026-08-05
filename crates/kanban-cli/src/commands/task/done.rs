use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{CompleteTaskRequest, CompleteTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct DoneArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(long)]
    pub(crate) force: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &DoneArgs,
) -> Result<(), CliFailure> {
    let task = client.complete_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &CompleteTaskRequest {
            actor: None,
            claim_token: args.claim_token.clone(),
            force: args.force,
            summary: None,
            result: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CompleteTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;
    use kanban_contract::CompleteTaskResponse;

    #[test]
    fn parses_task_done_and_complete_commands() {
        for command in ["done", "complete"] {
            let cli = Cli::try_parse_from([
                "kanban",
                "task",
                command,
                "default#1",
                "--claim-token",
                "claim_test",
            ])
            .expect("done args");
            let Command::Task {
                command: crate::commands::task::TaskCommand::Done(args),
            } = cli.command
            else {
                panic!("expected task done");
            };
            assert_eq!(args.task_ref, "default#1");
            assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
            assert!(!args.force);
        }
    }

    #[test]
    fn task_done_output_contract() {
        let fixture =
            include_str!("../../../../../schemas/fixtures/cli/task-done-output.v1.valid.json");
        let output: kanban_contract::CliTaskDoneOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "done");
        assert_eq!(
            serde_json::to_value(CompleteTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }

    #[test]
    fn task_complete_output_contract() {
        let fixture =
            include_str!("../../../../../schemas/fixtures/cli/task-complete-output.v1.valid.json");
        let output: kanban_contract::CliTaskCompleteOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "done");
        assert_eq!(
            serde_json::to_value(CompleteTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }
}
