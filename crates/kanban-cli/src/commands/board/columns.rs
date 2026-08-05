use clap::Args;
use kanban_contract::ListBoardColumnsResponse;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ColumnsArgs {
    pub(crate) board: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &ColumnsArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let board = args.board.as_deref().unwrap_or(&ctx.board);
    let columns = client.list_board_columns(board)?;
    if ctx.json {
        output::print_json(&ListBoardColumnsResponse { data: columns });
    } else {
        for column in columns {
            println!(
                "{} {}{}",
                column.status.as_str(),
                column.title,
                if column.hidden { " (hidden)" } else { "" }
            );
        }
    }
    Ok(())
}
