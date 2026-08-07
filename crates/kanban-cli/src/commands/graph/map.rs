use clap::Args;
use kanban_protocol::{BoardTaskMapQuery, BoardTaskMapResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct MapArgs {
    #[arg(long, default_value_t = true)]
    pub(crate) active_only: bool,
    #[arg(long, default_value_t = 1)]
    pub(crate) context_depth: usize,
    #[arg(long, default_value_t = 250)]
    pub(crate) limit_nodes: usize,
    #[arg(long, default_value_t = true)]
    pub(crate) include_done_context: bool,
    #[arg(long)]
    pub(crate) include_archived_context: bool,
    #[arg(long)]
    pub(crate) hide_isolated: bool,
}

pub(crate) fn run(ctx: &CliContext, args: &MapArgs) -> Result<(), CliFailure> {
    let value = ctx.client()?.board_task_map(
        &ctx.board,
        &BoardTaskMapQuery {
            active_only: args.active_only,
            context_depth: args.context_depth,
            limit_nodes: args.limit_nodes,
            include_done_context: args.include_done_context,
            include_archived_context: args.include_archived_context,
            hide_isolated: args.hide_isolated,
        },
    )?;
    if ctx.json {
        output::print_json(&BoardTaskMapResponse { data: value });
    } else {
        println!("nodes={} edges={}", value.nodes.len(), value.edges.len());
    }
    Ok(())
}
