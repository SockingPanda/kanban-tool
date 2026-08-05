use crate::{context::CliContext, error::CliFailure, output};
use clap::Args as ClapArgs;
use kanban_client::KanbanClient;
use kanban_contract::{
    CliLabelAtomHit, CliLabelAtomVectorHit, CliVectorLabelAtomHit, CliVectorQueryLabelAtomsOutput,
    VectorQuery,
};

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    pub(crate) q: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
    #[arg(long)]
    pub(crate) polarity: Option<String>,
    #[arg(long)]
    pub(crate) include_vector: bool,
}

pub(crate) fn run(ctx: &CliContext, client: &KanbanClient, args: &Args) -> Result<(), CliFailure> {
    let hits = client.query_vector_label_atoms(VectorQuery {
        board: ctx.board.clone(),
        q: args.q.clone(),
        limit: args.limit,
        embedding_model: None,
        polarity: args.polarity.clone(),
        include_vector: args.include_vector,
    })?;
    let data = hits
        .into_iter()
        .map(|hit| {
            let atom = CliLabelAtomHit {
                atom_id: hit.atom_id,
                label_id: hit.label_id,
                label_name: String::new(),
                board_id: hit.board_id,
                polarity: hit.polarity,
                kind: hit.kind,
                text: hit.text,
                ordinal: hit.ordinal,
                content_hash: hit.content_hash,
                embedding_model: hit.embedding_model,
                distance: hit.distance,
            };
            if args.include_vector {
                CliVectorLabelAtomHit::WithVector(CliLabelAtomVectorHit {
                    hit: atom,
                    vector: hit.vector,
                })
            } else {
                CliVectorLabelAtomHit::Hit(atom)
            }
        })
        .collect::<Vec<_>>();
    if ctx.json {
        output::print_json(&CliVectorQueryLabelAtomsOutput::new(data));
    } else {
        for hit in &data {
            if let CliVectorLabelAtomHit::Hit(atom) = hit {
                println!("{:.4} {}", atom.distance, atom.text);
            }
        }
    }
    Ok(())
}
