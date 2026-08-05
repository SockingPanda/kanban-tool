use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{HeartbeatTaskRequest, HeartbeatTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct HeartbeatArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: String,
    #[arg(long, default_value_t = 300_000)]
    pub(crate) ttl_ms: i64,
    #[arg(long)]
    pub(crate) note: Option<String>,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &HeartbeatArgs,
) -> Result<(), CliFailure> {
    let task = client.heartbeat_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &HeartbeatTaskRequest {
            actor: None,
            claim_token: args.claim_token.clone(),
            ttl_ms: args.ttl_ms,
            note: args.note.clone(),
        },
    )?;
    if ctx.json {
        output::print_json(&HeartbeatTaskResponse::new(task));
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
    fn parses_task_heartbeat_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "heartbeat",
            "default#1",
            "--claim-token",
            "claim_test",
            "--ttl-ms",
            "120000",
            "--note",
            "alive",
        ])
        .expect("heartbeat args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Heartbeat(args),
        } = cli.command
        else {
            panic!("expected task heartbeat");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token, "claim_test");
        assert_eq!(args.ttl_ms, 120_000);
        assert_eq!(args.note.as_deref(), Some("alive"));
    }
}
