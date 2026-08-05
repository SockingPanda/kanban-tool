use clap::Args;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) uri: String,
}

pub(crate) fn run(ctx: &CliContext, args: &ShowArgs) -> Result<(), CliFailure> {
    let entity = ctx.client()?.get_entity(&args.uri)?;
    if ctx.json {
        output::print_json(&kanban_contract::CliEntityShowOutput { data: entity });
    } else {
        println!("{} {} {}", entity.uri, entity.kind, entity.source_id);
    }
    Ok(())
}
