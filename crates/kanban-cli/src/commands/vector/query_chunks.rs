use crate::{context::CliContext, error::CliFailure, output};
use clap::Args as ClapArgs;
use kanban_client::KanbanClient;
use kanban_contract::{CliChunkRef, CliVectorChunkHit, CliVectorQueryChunksOutput, VectorQuery};

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    pub(crate) q: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
}

pub(crate) fn run(ctx: &CliContext, client: &KanbanClient, args: &Args) -> Result<(), CliFailure> {
    let hits = client.query_vector_chunks(VectorQuery {
        board: ctx.board.clone(),
        q: args.q.clone(),
        limit: args.limit,
        embedding_model: None,
        polarity: None,
        include_vector: false,
    })?;
    let data = hits
        .into_iter()
        .map(|hit| CliVectorChunkHit {
            chunk: CliChunkRef {
                uri: hit.id,
                entity_uri: hit.entity_uri.unwrap_or_default(),
                ordinal: 0,
                content_hash: Some(hit.content_hash),
            },
            score: hit.score,
            text: Some(hit.content),
            summary: None,
        })
        .collect::<Vec<_>>();
    if ctx.json {
        output::print_json(&CliVectorQueryChunksOutput::new(data));
    } else {
        for hit in &data {
            println!("{:.4} {}", hit.score, hit.chunk.uri);
        }
    }
    Ok(())
}
