use clap::Args;
use kanban_protocol::{ArchiveBoardRequest, ArchiveBoardResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ArchiveArgs {
    pub(crate) board: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &ArchiveArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let board = client.archive_board(
        args.board.as_deref().unwrap_or(&ctx.board),
        &ArchiveBoardRequest::default(),
    )?;
    if ctx.json {
        output::print_json(&ArchiveBoardResponse { data: board });
    } else {
        println!("{} {} 已归档", board.id, board.slug);
    }
    Ok(())
}
