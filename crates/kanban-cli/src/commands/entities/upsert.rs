use clap::Args;

use kanban_client::EntityUpsertRequest;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct UpsertArgs {
    #[arg(long)]
    pub(crate) uri: String,
    #[arg(long)]
    pub(crate) kind: String,
    #[arg(long)]
    pub(crate) source_table: String,
    #[arg(long)]
    pub(crate) source_id: String,
    #[arg(long)]
    pub(crate) task_id: Option<String>,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) summary: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &UpsertArgs) -> Result<(), CliFailure> {
    let entity = ctx.client()?.upsert_entity(EntityUpsertRequest {
        uri: args.uri.clone(),
        kind: args.kind.clone(),
        source_table: args.source_table.clone(),
        source_id: args.source_id.clone(),
        board: Some(ctx.board.clone()),
        task_id: args.task_id.clone(),
        title: args.title.clone(),
        summary: args.summary.clone(),
        content_hash: None,
        archived_at: None,
    })?;
    if ctx.json {
        output::print_json(&kanban_protocol::CliEntityShowOutput { data: entity });
    } else {
        println!("{} {} {}", entity.uri, entity.kind, entity.source_id);
    }
    Ok(())
}
