use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::CliRunShowOutput;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) run_id: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ShowArgs,
) -> Result<(), CliFailure> {
    let run = client.get_run(&args.run_id)?;
    if ctx.json {
        output::print_json(&CliRunShowOutput::new(run));
    } else {
        let exit_code = run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "-".to_owned());
        println!(
            "{} [{}] task={} exit={}",
            run.id,
            run.status.as_str(),
            run.task_id,
            exit_code
        );
    }
    Ok(())
}
