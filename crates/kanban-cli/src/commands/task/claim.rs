use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{ClaimTaskRequest, ClaimTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ClaimArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = 300_000)]
    pub(crate) ttl_ms: i64,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ClaimArgs,
) -> Result<(), CliFailure> {
    let claim = client.claim_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &ClaimTaskRequest {
            actor: None,
            ttl_ms: args.ttl_ms,
            worker_profile: None,
            metadata: None,
        },
    )?;
    if ctx.json {
        output::print_json(&ClaimTaskResponse::new(claim));
    } else {
        println!("Claimed {} token={}", claim.task.id, claim.claim_token);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_task_claim_command() {
        let cli =
            Cli::try_parse_from(["kanban", "task", "claim", "default#1", "--ttl-ms", "120000"])
                .expect("claim args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Claim(args),
        } = cli.command
        else {
            panic!("expected task claim");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.ttl_ms, 120_000);
    }
}
