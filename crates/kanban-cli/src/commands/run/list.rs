use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::CliRunsOutput;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    pub(crate) task_ref: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ListArgs,
) -> Result<(), CliFailure> {
    let runs = client.list_runs_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&CliRunsOutput::new(runs));
    } else {
        for run in runs {
            let exit_code = run
                .exit_code
                .map_or_else(|| "-".to_owned(), |code| code.to_string());
            println!(
                "{} [{}] task={} exit={}",
                run.id,
                run.status.as_str(),
                run.task_id,
                exit_code
            );
        }
    }
    Ok(())
}
