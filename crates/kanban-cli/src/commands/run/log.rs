use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{CliRunLog, CliRunLogsOutput};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct LogArgs {
    /// 全局 r_... run ID。
    pub(crate) run_id: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &LogArgs,
) -> Result<(), CliFailure> {
    let log = client.get_run_log(&args.run_id)?;
    if ctx.json {
        output::print_json(&CliRunLogsOutput::new(CliRunLog {
            run_id: log.run_id,
            content: log.content,
            truncated: log.truncated,
            tail_bytes: None,
        }));
    } else {
        print!("{}", log.content);
    }
    Ok(())
}
