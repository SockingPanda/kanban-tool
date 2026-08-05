use clap::Args;
use kanban_protocol::ListBoardsResponse;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long)]
    pub(crate) include_archived: bool,
}

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let boards = client.list_boards(args.include_archived)?;
    if ctx.json {
        output::print_json(&ListBoardsResponse { data: boards });
    } else {
        for board in boards {
            println!("{} {} {}", board.id, board.slug, board.name);
        }
    }
    Ok(())
}
