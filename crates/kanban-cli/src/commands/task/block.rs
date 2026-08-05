use std::{io::Read, path::PathBuf};

use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{BlockTaskRequest, BlockTaskResponse};

use crate::{context::CliContext, error::CliFailure, output};

pub(crate) const MAX_TEXT_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Args)]
pub(crate) struct BlockArgs {
    pub(crate) task_ref: String,
    #[arg(
        required_unless_present = "reason_file",
        conflicts_with = "reason_file"
    )]
    pub(crate) reason: Option<String>,
    #[arg(
        long,
        value_name = "PATH|->",
        required_unless_present = "reason",
        conflicts_with = "reason"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(long)]
    pub(crate) force: bool,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &BlockArgs,
) -> Result<(), CliFailure> {
    let reason = block_reason(args)?;
    let task = client.block_task_by_selector(
        &ctx.board,
        &args.task_ref,
        &BlockTaskRequest {
            actor: None,
            reason,
            claim_token: args.claim_token.clone(),
            force: args.force,
        },
    )?;
    if ctx.json {
        output::print_json(&BlockTaskResponse::new(task));
    } else {
        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
    }
    Ok(())
}

pub(crate) fn block_reason(args: &BlockArgs) -> Result<String, CliFailure> {
    let reason = match (&args.reason, &args.reason_file) {
        (Some(reason), None) => reason.clone(),
        (None, Some(path)) if path.as_os_str() == "-" => {
            let stdin = std::io::stdin();
            read_limited_text(stdin.lock(), "--reason-file -")?
        }
        (None, Some(path)) => {
            let file = std::fs::File::open(path).map_err(|error| CliFailure {
                code: "invalid_input",
                message: format!("failed to read --reason-file {}: {error}", path.display()),
                exit_code: 2,
            })?;
            read_limited_text(file, &format!("--reason-file {}", path.display()))?
        }
        _ => {
            return Err(CliFailure {
                code: "invalid_input",
                message: "block requires exactly one reason or --reason-file".to_owned(),
                exit_code: 2,
            });
        }
    };
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "block reason is required".to_owned(),
            exit_code: 2,
        });
    }
    Ok(reason.to_owned())
}

fn read_limited_text(reader: impl Read, label: &str) -> Result<String, CliFailure> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_TEXT_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| CliFailure {
            code: "invalid_input",
            message: format!("failed to read {label}: {error}"),
            exit_code: 2,
        })?;
    if bytes.len() > MAX_TEXT_INPUT_BYTES {
        return Err(CliFailure {
            code: "invalid_input",
            message: format!("{label} exceeds the 1 MiB input limit"),
            exit_code: 2,
        });
    }
    String::from_utf8(bytes).map_err(|error| CliFailure {
        code: "invalid_input",
        message: format!("{label} must be UTF-8: {error}"),
        exit_code: 2,
    })
}

#[cfg(test)]
mod tests {
    use super::{MAX_TEXT_INPUT_BYTES, block_reason, read_limited_text};
    use crate::{Cli, Command};
    use clap::Parser;
    use kanban_protocol::BlockTaskResponse;

    #[test]
    fn parses_task_block_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "block",
            "default#1",
            "waiting",
            "--claim-token",
            "claim_test",
        ])
        .expect("block args");
        let Command::Task {
            command: crate::commands::task::TaskCommand::Block(args),
        } = cli.command
        else {
            panic!("expected task block");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(block_reason(&args).unwrap(), "waiting");
        assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
        assert!(!args.force);
    }

    #[test]
    fn block_reason_file_input_is_bounded() {
        let accepted = read_limited_text(
            std::io::Cursor::new(vec![b'x'; MAX_TEXT_INPUT_BYTES]),
            "reason",
        )
        .unwrap();
        assert_eq!(accepted.len(), MAX_TEXT_INPUT_BYTES);
        let error = read_limited_text(
            std::io::Cursor::new(vec![b'x'; MAX_TEXT_INPUT_BYTES + 1]),
            "reason",
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_input");
        assert!(error.message.contains("1 MiB"));
    }

    #[test]
    fn task_block_output_contract() {
        let fixture =
            include_str!("../../../../../schemas/fixtures/cli/task-block-output.v1.valid.json");
        let output: kanban_protocol::CliTaskBlockOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "blocked");
        assert_eq!(
            serde_json::to_value(BlockTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }
}
