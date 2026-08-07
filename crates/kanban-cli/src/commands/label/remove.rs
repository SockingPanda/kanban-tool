use clap::Args;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    pub(crate) task_ref: String,
    pub(crate) label: String,
}

pub(crate) fn run(ctx: &CliContext, args: &RemoveArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let task = client.remove_task_label_by_selector(&ctx.board, &args.task_ref, &args.label)?;
    if ctx.json {
        output::print_json(&kanban_protocol::cli_labels::CliLabelRemoveOutput { data: task });
    } else {
        println!(
            "{} labels={}",
            task.task_ref,
            task.labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    Ok(())
}
