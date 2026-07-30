use std::io::Read;
use std::{path::PathBuf, process};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use kanban_derived_io::{
    board_id, connect_file, current_last_event_id, derived_status_by_name,
    has_pending_vector_outbox_for_board, label_atom_index_status_from_base,
    rebuild_lancedb_chunks_with_store, rebuild_lancedb_label_atoms_with_store,
    sync_lancedb_chunks_with_store, sync_lancedb_label_atoms_with_store,
};
use kanban_helper_protocol::HelperEnvelope;
use kanban_indexer::LANCEDB_CHUNKS_STORE;
use kanban_vector::{
    ChunkVectorStore, EmbeddingProvider, LabelAtomQuery, LabelAtomVectorQuery,
    LabelAtomVectorStore, VectorQuery, VectorStoreBackend, VectorStoreStatus,
};
use kanban_vector_lancedb::{LanceDbConfig, LanceDbStore, OllamaEmbeddingProvider};
use kanban_vector_lancedb::{
    decode_vector_projection_request, vector_helper_check_provider_response,
    vector_helper_embed_query_response, vector_helper_error_response,
    vector_helper_handshake_response, vector_helper_query_chunks_response,
    vector_helper_query_label_atom_vectors_response, vector_helper_query_label_atoms_response,
    vector_helper_status_response, vector_projection_descriptor_response,
    vector_projection_invalid_request_response, vector_projection_unavailable_response,
};
use serde::Serialize;
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
    Projection(ProjectionArgs),
    Status(StoreArgs),
    CheckProvider(ProviderArgs),
    Rebuild(StoreArgs),
    Sync(StoreArgs),
    QueryChunks(QueryChunksArgs),
    QueryLabelAtoms(QueryLabelAtomsArgs),
    #[command(name = "label-atoms-status")]
    LabelAtomsStatus(StoreArgs),
    #[command(name = "rebuild-label-atoms")]
    RebuildLabelAtoms(StoreArgs),
    #[command(name = "sync-label-atoms")]
    SyncLabelAtoms(StoreArgs),
    EmbedQuery(EmbedQueryArgs),
}

#[derive(Debug, Parser)]
struct ProjectionArgs {
    #[arg(long)]
    db: PathBuf,
    #[arg(long = "vector-config", alias = "config")]
    vector_config: Option<PathBuf>,
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
    #[arg(long)]
    board_id: String,
}

#[derive(Debug, Parser)]
struct QueryLabelAtomsArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(
        long,
        conflicts_with = "vector_json",
        required_unless_present = "vector_json"
    )]
    text: Option<String>,
    #[arg(
        long = "vector-json",
        conflicts_with = "text",
        required_unless_present = "text"
    )]
    vector_json: Option<String>,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    board_id: Option<String>,
    #[arg(long = "embedding-model")]
    embedding_model: Option<String>,
    #[arg(long)]
    polarity: Option<String>,
    #[arg(long = "include-vector")]
    include_vector: bool,
}

#[derive(Debug, Parser)]
struct EmbedQueryArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    text: String,
}

fn main() {
    if let Err(error) = run() {
        let payload = vector_helper_error_response(error.to_string());
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
        Command::Handshake => {
            print_payload(vector_helper_handshake_response(env!("CARGO_PKG_VERSION")))
        }
        Command::Projection(args) => run_projection(args),
        Command::Status(args) => {
            print_payload(vector_helper_status_response(vector_status(&args)?))
        }
        Command::CheckProvider(args) => {
            provider(&args)?.check()?;
            print_payload(vector_helper_check_provider_response())
        }
        Command::Rebuild(args) => {
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                rebuild_lancedb_chunks_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::Sync(args) => {
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                sync_lancedb_chunks_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::QueryChunks(args) => {
            let conn = connect_file(&args.store.db)?;
            let resolved_board_id = board_id(&conn, &args.store.board)?;
            if resolved_board_id != args.board_id {
                bail!(
                    "query chunk board mismatch: --board resolved to {resolved_board_id}, got --board-id {}",
                    args.board_id
                );
            }
            let store = configured_store(&args.store)?;
            let hits = store.query(&VectorQuery {
                text: args.text,
                limit: args.limit,
                board_id: resolved_board_id,
            })?;
            print_payload(vector_helper_query_chunks_response(hits))
        }
        Command::QueryLabelAtoms(args) => {
            if let Some(vector_json) = args.vector_json {
                let vector = parse_vector_json(&vector_json)?;
                let store = configured_store(&args.store)?;
                let hits = store.query_label_atoms_by_vector(&LabelAtomVectorQuery {
                    vector,
                    limit: args.limit,
                    board_id: args.board_id,
                    embedding_model: args.embedding_model,
                    polarity: args.polarity,
                    include_vector: args.include_vector,
                })?;
                print_payload(vector_helper_query_label_atom_vectors_response(hits))
            } else {
                let store = configured_store(&args.store)?;
                let hits = store.query_label_atoms(&LabelAtomQuery {
                    text: args.text.unwrap_or_default(),
                    limit: args.limit,
                    board_id: args.board_id,
                    embedding_model: args.embedding_model,
                    polarity: args.polarity,
                })?;
                print_payload(vector_helper_query_label_atoms_response(hits))
            }
        }
        Command::LabelAtomsStatus(args) => {
            print_payload(vector_helper_status_response(label_atom_status(&args)?))
        }
        Command::RebuildLabelAtoms(args) => {
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                rebuild_lancedb_label_atoms_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::SyncLabelAtoms(args) => {
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                sync_lancedb_label_atoms_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::EmbedQuery(args) => {
            let provider = provider_from_store_args(&args.store)?;
            print_payload(vector_helper_embed_query_response(
                provider.embed(&args.text)?,
            ))
        }
    }
}

fn run_projection(args: ProjectionArgs) -> Result<()> {
    const MAX_PROJECTION_STDIN_BYTES: u64 = 32 * 1024 * 1024;

    let _ = (&args.db, &args.vector_config);
    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_PROJECTION_STDIN_BYTES + 1)
        .read_to_end(&mut input)
        .context("failed to read vector projection request")?;
    let response = if input.len() as u64 > MAX_PROJECTION_STDIN_BYTES {
        vector_projection_invalid_request_response()
    } else {
        match decode_vector_projection_request(&input) {
            Ok(kanban_contract::VectorProjectionHelperRequest::Descriptor(request)) => {
                vector_projection_descriptor_response(request.request_id)
            }
            Ok(request) => vector_projection_unavailable_response(&request),
            Err(_) => vector_projection_invalid_request_response(),
        }
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

fn parse_vector_json(vector_json: &str) -> Result<Vec<f32>> {
    let mut deserializer = serde_json::Deserializer::from_str(vector_json);
    let vector = serde_path_to_error::deserialize(&mut deserializer).map_err(|err| {
        let path = vector_json_path(&err.path().to_string());
        anyhow!(
            "invalid --vector-json payload at {path}: {}",
            err.into_inner()
        )
    })?;
    deserializer.end().map_err(|err| {
        anyhow!(
            "invalid --vector-json payload at {}: {err}",
            vector_json_path(".")
        )
    })?;
    Ok(vector)
}

fn vector_json_path(path: &str) -> String {
    if path == "." {
        "<root>".to_owned()
    } else {
        path.to_owned()
    }
}

fn label_atom_status(args: &StoreArgs) -> Result<VectorStoreStatus> {
    let conn = connect_file(&args.db)?;
    let board_id = board_id(&conn, &args.board)?;
    let mut status = match resolved_config(args)? {
        Some(config) => VectorStoreStatus::new(
            "lancedb-label-atoms",
            true,
            format!(
                "LanceDB label atom helper enabled for Ollama endpoint {}, model {} ({} dimensions)",
                config.endpoint, config.model, config.dimensions
            ),
        ),
        None => {
            let mut status = LanceDbStore::connect(LanceDbConfig::degraded(
                kanban_local::vector_store_path(args.db.clone()),
            ))?
            .status();
            status.backend = "lancedb-label-atoms".to_owned();
            status.message = "LanceDB label atom helper configured without an embedding provider; label atom retrieval degraded".to_owned();
            status
        }
    };
    if !status
        .diagnostics
        .iter()
        .any(|code| code == "label_atom_helper")
    {
        status.diagnostics.push("label_atom_helper".to_owned());
    }
    label_atom_index_status_from_base(&conn, &board_id, status).map_err(Into::into)
}

fn provider_from_store_args(args: &StoreArgs) -> Result<OllamaEmbeddingProvider> {
    let config = kanban_local::resolved_vector_config(args.vector_config.as_deref())?
        .context("vector config is required")?;
    provider(&ProviderArgs {
        endpoint: config.endpoint,
        model: config.model,
        dimensions: config.dimensions,
    })
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
