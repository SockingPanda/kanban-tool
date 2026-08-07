use clap::Args;
use kanban_protocol::GetBoardResponse;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) board: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &ShowArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let board = client.get_board(args.board.as_deref().unwrap_or(&ctx.board))?;
    if ctx.json {
        output::print_json(&GetBoardResponse { data: board });
    } else {
        println!("{} {} {}", board.id, board.slug, board.name);
        if let Some(description) = board.description.as_deref() {
            println!("描述：{description}");
        }
        if let Some(archived_at) = board.archived_at {
            println!("归档时间：{archived_at}");
        }
    }
    Ok(())
}
