use clap::Args;
use kanban_contract::CreateBoardLabelRequest;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct CreateArgs {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) color: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &CreateArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let label = client.create_board_label(
        &ctx.board,
        &CreateBoardLabelRequest {
            name: args.name.clone(),
            color: args.color.clone(),
        },
    )?;
    if ctx.json {
        output::print_json(&kanban_contract::cli_labels::CliLabelCreateOutput { data: label });
    } else {
        println!("{} {}", label.id, label.name);
    }
    Ok(())
}
