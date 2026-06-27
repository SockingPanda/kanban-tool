use std::{path::PathBuf, process};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use kanban_derived_io::{
    HelperEnvelope, board_id, connect_file, current_last_event_id, derived_status_by_name,
    has_pending_vector_outbox_for_board, rebuild_lancedb_chunks_with_store,
    sync_lancedb_chunks_with_store,
};
use kanban_indexer::LANCEDB_CHUNKS_STORE;
use kanban_vector::{
    ChunkVectorStore, LabelAtomQuery, LabelAtomVectorStore, VectorQuery, VectorStoreBackend,
    VectorStoreStatus,
};
use kanban_vector_lancedb::{LanceDbConfig, LanceDbStore, OllamaEmbeddingProvider};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(name = "kanban-vector-lancedb")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Handshake,
    Status(StoreArgs),
    CheckProvider(ProviderArgs),
    Rebuild(StoreArgs),
    Sync(StoreArgs),
    QueryChunks(QueryChunksArgs),
    QueryLabelAtoms(QueryLabelAtomsArgs),
}

#[derive(Debug, Parser)]
struct StoreArgs {
    #[arg(long)]
    db: PathBuf,
    #[arg(long)]
    board: String,
    #[arg(long = "vector-config", alias = "config")]
    vector_config: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ProviderArgs {
    #[arg(long)]
    endpoint: String,
    #[arg(long)]
    model: String,
    #[arg(long)]
    dimensions: usize,
}

#[derive(Debug, Parser)]
struct QueryChunksArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    text: String,
    #[arg(long, default_value_t = 10)]
    limit: usize,
}

#[derive(Debug, Parser)]
struct QueryLabelAtomsArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    text: String,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    board_id: Option<String>,
    #[arg(long)]
    polarity: Option<String>,
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
            "helper": "kanban-vector-lancedb",
            "protocol": HelperEnvelope::PROTOCOL,
            "version": env!("CARGO_PKG_VERSION"),
        })),
        Command::Status(args) => print_payload(vector_status(&args)?),
        Command::CheckProvider(args) => {
            provider(&args)?.check()?;
            print_payload(json!({"ok": true}))
        }
        Command::Rebuild(args) => {
            let store = configured_store(&args)?;
            print_payload(rebuild_lancedb_chunks_with_store(
                &args.db,
                &args.board,
                &store,
            )?)
        }
        Command::Sync(args) => {
            let store = configured_store(&args)?;
            print_payload(sync_lancedb_chunks_with_store(
                &args.db,
                &args.board,
                &store,
            )?)
        }
        Command::QueryChunks(args) => {
            let store = configured_store(&args.store)?;
            let hits = store.query(&VectorQuery {
                text: args.text,
                limit: args.limit,
            })?;
            print_payload(hits)
        }
        Command::QueryLabelAtoms(args) => {
            let store = configured_store(&args.store)?;
            let hits = store.query_label_atoms(&LabelAtomQuery {
                text: args.text,
                limit: args.limit,
                board_id: args.board_id,
                embedding_model: None,
                polarity: args.polarity,
            })?;
            print_payload(hits)
        }
    }
}

fn vector_status(args: &StoreArgs) -> Result<VectorStoreStatus> {
    let conn = connect_file(&args.db)?;
    let board_id = board_id(&conn, &args.board)?;
    let mut status = match resolved_config(args)? {
        Some(config) => VectorStoreStatus::new(
            "lancedb",
            true,
            format!(
                "LanceDB vector helper enabled for Ollama endpoint {}, model {} ({} dimensions)",
                config.endpoint, config.model, config.dimensions
            ),
        ),
        None => LanceDbStore::connect(LanceDbConfig::degraded(kanban_local::vector_store_path(
            args.db.clone(),
        )))?
        .status(),
    };
    let state = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let board_dirty = has_pending_vector_outbox_for_board(&conn, &board_id, current_last_event_id)?;
    status.dirty = Some(state.dirty);
    status.board_dirty = Some(board_dirty);
    if !status.enabled {
        push_diagnostic(&mut status, "vector_store_disabled");
    }
    if state.dirty {
        push_diagnostic(&mut status, "vector_dirty");
    }
    if board_dirty {
        push_diagnostic(&mut status, "vector_board_dirty");
    }
    if state.last_error.is_some() {
        push_diagnostic(&mut status, "vector_error");
    }
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

fn configured_store(args: &StoreArgs) -> Result<LanceDbStore> {
    let Some(config) = resolved_config(args)? else {
        bail!(
            "vector helper requires a configured embedding provider; run `kanban vector configure` or pass --vector-config"
        )
    };
    let provider = Arc::new(provider_from_config(&config)?);
    LanceDbStore::connect(LanceDbConfig::new(
        kanban_local::vector_store_path(args.db.clone()),
        provider,
    ))
    .map_err(Into::into)
}

fn resolved_config(args: &StoreArgs) -> Result<Option<kanban_local::VectorConfig>> {
    kanban_local::resolved_vector_config(args.vector_config.as_deref())
        .with_context(|| "failed to read vector config")
}

fn provider(args: &ProviderArgs) -> Result<OllamaEmbeddingProvider> {
    OllamaEmbeddingProvider::new(args.endpoint.clone(), args.model.clone(), args.dimensions)
        .map_err(Into::into)
}

fn provider_from_config(config: &kanban_local::VectorConfig) -> Result<OllamaEmbeddingProvider> {
    if config.provider != "ollama" {
        bail!("unsupported vector provider in config: {}", config.provider);
    }
    OllamaEmbeddingProvider::new(
        config.endpoint.clone(),
        config.model.clone(),
        config.dimensions,
    )
    .map_err(Into::into)
}

fn push_diagnostic(status: &mut VectorStoreStatus, code: &str) {
    if !status.diagnostics.iter().any(|value| value == code) {
        status.diagnostics.push(code.to_owned());
    }
}

fn print_payload(payload: impl Serialize) -> Result<()> {
    println!("{}", HelperEnvelope::new(payload)?.to_json()?);
    Ok(())
}
