use clap::Args;
use kanban_protocol::{AddTaskLabelRequest, cli_labels::CliLabelAddResult};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    pub(crate) task_ref: String,
    #[arg(required = true, num_args = 1..)]
    pub(crate) labels: Vec<String>,
    #[arg(long)]
    pub(crate) create_missing: bool,
}

pub(crate) fn run(ctx: &CliContext, args: &AddArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let response = client.add_task_labels_by_selector(
        &ctx.board,
        &args.task_ref,
        &AddTaskLabelRequest {
            name: None,
            names: Some(args.labels.clone()),
            create_missing: args.create_missing,
            actor: None,
        },
    )?;
    if ctx.json {
        let data = match response.meta {
            Some(meta) => CliLabelAddResult::WithCreated(
                kanban_protocol::cli_labels::CliLabelAddWithCreated {
                    task: response.data,
                    created_labels: meta.created_labels,
                },
            ),
            None => CliLabelAddResult::Task(response.data),
        };
        output::print_json(&kanban_protocol::cli_labels::CliLabelAddOutput { data });
    } else {
        println!(
            "{} labels={}",
            response.data.task_ref,
            response
                .data
                .labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    Ok(())
}
