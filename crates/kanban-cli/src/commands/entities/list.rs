use clap::Args;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let entities =
        ctx.client()?
            .list_entities(Some(&ctx.board), args.kind.as_deref(), args.limit)?;
    if ctx.json {
        output::print_json(&kanban_contract::CliEntityListOutput { data: entities });
    } else {
        for entity in entities {
            println!("{} {} {}", entity.uri, entity.kind, entity.source_id);
        }
    }
    Ok(())
}
