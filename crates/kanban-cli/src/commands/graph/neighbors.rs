use clap::Args;

use kanban_protocol::{GraphNeighborsQuery, LimitMeta};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct NeighborsArgs {
    pub(crate) entity_uri: String,
    #[arg(long)]
    pub(crate) predicate: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

pub(crate) fn run(ctx: &CliContext, args: &NeighborsArgs) -> Result<(), CliFailure> {
    let response = ctx.client()?.graph_neighbors(&GraphNeighborsQuery {
        board: ctx.board.clone(),
        entity_uri: args.entity_uri.clone(),
        predicate: args.predicate.clone(),
        limit: args.limit,
    })?;
    let relations = response
        .data
        .into_iter()
        .map(|relation| kanban_protocol::cli_helpers::CliGraphRelation {
            subject_uri: relation.subject_uri,
            predicate: relation.predicate,
            object_uri: relation.object_uri,
            graph_uri: relation.graph_uri,
            provenance: kanban_protocol::cli_helpers::CliGraphRelationProvenance {
                source_table: relation.provenance.source_table,
                source_id: relation.provenance.source_id,
                source_event_id: relation.provenance.source_event_id,
                authoritative_store: relation.provenance.authoritative_store,
            },
            metadata: relation.metadata,
            created_at: relation.created_at,
            updated_at: relation.updated_at,
        })
        .collect::<Vec<_>>();
    if ctx.json {
        output::print_json(&kanban_protocol::cli_helpers::CliGraphNeighborsOutput {
            data: relations,
        });
    } else {
        for relation in relations {
            println!(
                "{} {} {}",
                relation.subject_uri, relation.predicate, relation.object_uri
            );
        }
    }
    let _ = LimitMeta { limit: args.limit };
    Ok(())
}
