use clap::Args;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {}

pub(crate) fn run(ctx: &CliContext, _args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let labels = client.list_board_labels(&ctx.board)?;
    if ctx.json {
        output::print_json(&kanban_protocol::cli_labels::CliLabelListOutput { data: labels });
    } else {
        for label in labels {
            println!(
                "{} {}{}",
                label.id,
                label.name,
                label
                    .color
                    .as_deref()
                    .map(|color| format!(" ({color})"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}
