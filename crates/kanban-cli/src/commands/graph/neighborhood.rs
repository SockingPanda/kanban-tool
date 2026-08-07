use clap::Args;
use kanban_protocol::{TaskNeighborhoodQuery, TaskNeighborhoodResponse};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct NeighborhoodArgs {
    pub(crate) task_id: String,
    #[arg(long, default_value_t = 1)]
    pub(crate) depth: usize,
    #[arg(long, default_value_t = 250)]
    pub(crate) limit_nodes: usize,
    #[arg(long)]
    pub(crate) include_archived_context: bool,
}

pub(crate) fn run(ctx: &CliContext, args: &NeighborhoodArgs) -> Result<(), CliFailure> {
    let value = ctx.client()?.task_neighborhood(
        &args.task_id,
        &TaskNeighborhoodQuery {
            depth: args.depth,
            limit_nodes: args.limit_nodes,
            include_archived_context: args.include_archived_context,
        },
    )?;
    if ctx.json {
        output::print_json(&TaskNeighborhoodResponse { data: value });
    } else {
        println!("nodes={} edges={}", value.nodes.len(), value.edges.len());
    }
    Ok(())
}
