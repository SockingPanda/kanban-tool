use std::{path::PathBuf, process};

use anyhow::Result;
use clap::{Parser, Subcommand};
use kanban_derived_io::{
    HelperEnvelope, board_id, connect_file, current_last_event_id, derived_status_by_name,
    has_pending_graph_outbox_for_board, rebuild_oxigraph_with_store, sync_oxigraph_with_store,
};
use kanban_entity::{EntityUri, Predicate};
use kanban_graph::{GraphStoreStatus, RelationGraph};
use kanban_graph_oxigraph::OxigraphStore;
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Parser)]
#[command(name = "kanban-graph-oxigraph")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Handshake,
    Status(StoreArgs),
    Rebuild(StoreArgs),
    Sync(StoreArgs),
    Neighbors(NeighborsArgs),
    Query(QueryArgs),
}

#[derive(Debug, Parser)]
struct StoreArgs {
    #[arg(long)]
    db: PathBuf,
    #[arg(long)]
    board: String,
}

#[derive(Debug, Parser)]
struct NeighborsArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    entity_uri: String,
    #[arg(long)]
    predicate: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Parser)]
struct QueryArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    sparql: String,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    code: &'static str,
    message: String,
}

fn main() {
    if let Err(error) = run() {
        let payload = ErrorPayload {
            code: "helper_error",
            message: error.to_string(),
        };
        if let Ok(envelope) = HelperEnvelope::new(payload).and_then(|envelope| envelope.to_json()) {
            println!("{envelope}");
        } else {
            eprintln!("{error}");
        }
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Handshake => print_payload(json!({
            "helper": "kanban-graph-oxigraph",
            "protocol": HelperEnvelope::PROTOCOL,
            "version": env!("CARGO_PKG_VERSION"),
        })),
        Command::Status(args) => print_payload(graph_status(&args)?),
        Command::Rebuild(args) => {
            let graph = graph_store(&args)?;
            print_payload(rebuild_oxigraph_with_store(&args.db, &args.board, &graph)?)
        }
        Command::Sync(args) => {
            let graph = graph_store(&args)?;
            print_payload(sync_oxigraph_with_store(&args.db, &args.board, &graph)?)
        }
        Command::Neighbors(args) => {
            let graph = graph_store(&args.store)?;
            let uri = EntityUri::new(args.entity_uri)?;
            let predicate = args.predicate.as_deref().map(parse_predicate).transpose()?;
            print_payload(graph.neighbors(&uri, predicate, args.limit)?)
        }
        Command::Query(args) => {
            let graph = graph_store(&args.store)?;
            print_payload(graph.query(&args.sparql, args.limit)?)
        }
    }
}

fn graph_status(args: &StoreArgs) -> Result<GraphStoreStatus> {
    let conn = connect_file(&args.db)?;
    let board_id = board_id(&conn, &args.board)?;
    let graph = graph_store(args)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let board_dirty = has_pending_graph_outbox_for_board(&conn, &board_id, current_last_event_id)?;
    let mut status = graph.status();
    status.message = format!(
        "{}; dirty={} last_event_id={} board_dirty={} last_error={}",
        status.message,
        state.dirty,
        state.last_event_id,
        board_dirty,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

fn graph_store(args: &StoreArgs) -> Result<OxigraphStore> {
    OxigraphStore::open(kanban_local::graph_store_path(args.db.clone())).map_err(Into::into)
}

fn parse_predicate(value: &str) -> Result<Predicate> {
    match value {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        _ => anyhow::bail!("unknown predicate: {value}"),
    }
}

fn print_payload(payload: impl Serialize) -> Result<()> {
    println!("{}", HelperEnvelope::new(payload)?.to_json()?);
    Ok(())
}
