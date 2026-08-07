use clap::Args;
use kanban_protocol::{CreateBoardRequest, CreateBoardResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct CreateArgs {
    pub(crate) slug: String,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &CreateArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let board = client.create_board(CreateBoardRequest {
        slug: args.slug.clone(),
        name: args.name.clone(),
        description: args.description.clone(),
        actor: None,
    })?;
    if ctx.json {
        output::print_json(&CreateBoardResponse { data: board });
    } else {
        println!("{} {} {}", board.id, board.slug, board.name);
    }
    Ok(())
}
