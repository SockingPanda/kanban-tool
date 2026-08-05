use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{SubmitReviewTaskRequest, SubmitReviewTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(long)]
    pub(crate) force: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ReviewArgs,
) -> Result<(), CliFailure> {
    let task = client.submit_review_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &SubmitReviewTaskRequest {
            actor: None,
            claim_token: args.claim_token.clone(),
            force: args.force,
            summary: None,
        },
    )?;
    if ctx.json {
        output::print_json(&SubmitReviewTaskResponse::new(task));
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
    fn parses_task_review_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "review",
            "default#1",
            "--claim-token",
            "claim_test",
        ])
        .expect("review args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Review(args),
        } = cli.command
        else {
            panic!("expected task review");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
        assert!(!args.force);

        let cli = Cli::try_parse_from(["kanban", "task", "review", "t_global", "--force"])
            .expect("forced review args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Review(args),
        } = cli.command
        else {
            panic!("expected forced task review");
        };
        assert_eq!(args.claim_token, None);
        assert!(args.force);
    }
}
