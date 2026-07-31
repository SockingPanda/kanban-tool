use std::io::{Read, Write};
use std::{path::PathBuf, process};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use kanban_derived_io::{
    connect_file, ensure_legacy_projection_control, rebuild_lancedb_chunks_with_store,
    rebuild_lancedb_label_atoms_with_store, sync_lancedb_chunks_with_store,
    sync_lancedb_label_atoms_with_store,
};
use kanban_helper_protocol::HelperEnvelope;
use kanban_indexer::{LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE};
use kanban_vector::{
    EmbeddingProvider, LabelAtomQuery, LabelAtomVectorQuery, VectorQuery, VectorStoreBackend,
    VectorStoreStatus,
};
use kanban_vector_lancedb::{
    ActiveLanceProjectionReader, LanceDbConfig, LanceDbStore, OllamaEmbeddingProvider,
    VectorProjectionBackend,
};
use kanban_vector_lancedb::{
    decode_vector_projection_request, vector_helper_build_identity,
    vector_helper_check_provider_response, vector_helper_embed_query_response,
    vector_helper_error_response, vector_helper_handshake_response,
    vector_helper_query_chunks_response, vector_helper_query_label_atom_vectors_response,
    vector_helper_query_label_atoms_response, vector_helper_status_response,
    vector_projection_descriptor_response, vector_projection_invalid_request_response,
    vector_projection_unavailable_response,
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
    #[command(name = "__build-identity", hide = true)]
    BuildIdentity,
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
    board_id: Option<String>,
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
        Command::BuildIdentity => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(vector_helper_build_identity().as_bytes())
                .context("failed to write helper build identity")?;
            stdout
                .flush()
                .context("failed to flush helper build identity")?;
            Ok(())
        }
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
            require_legacy_control(&args, LANCEDB_CHUNKS_STORE)?;
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                rebuild_lancedb_chunks_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::Sync(args) => {
            require_legacy_control(&args, LANCEDB_CHUNKS_STORE)?;
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                sync_lancedb_chunks_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::QueryChunks(args) => {
            let preflight_board_id =
                ActiveLanceProjectionReader::resolve_board(&args.store.db, &args.store.board)?;
            if let Some(requested_board_id) = args.board_id.as_deref()
                && preflight_board_id != requested_board_id
            {
                bail!(
                    "query chunk board mismatch: --board resolved to {preflight_board_id}, got --board-id {}",
                    requested_board_id
                );
            }
            let reader = configured_active_reader_for_board(
                &args.store,
                LANCEDB_CHUNKS_STORE,
                &preflight_board_id,
            )?;
            let resolved_board_id = reader
                .resolved_board_id()
                .expect("board-scoped reader must retain the resolved board")
                .to_owned();
            let hits = reader.query_chunks(&VectorQuery {
                text: args.text,
                limit: args.limit,
                board_id: resolved_board_id,
            })?;
            print_payload(vector_helper_query_chunks_response(hits))
        }
        Command::QueryLabelAtoms(args) => {
            let vector = args
                .vector_json
                .as_deref()
                .map(parse_vector_json)
                .transpose()?;
            let preflight_board_id =
                ActiveLanceProjectionReader::resolve_board(&args.store.db, &args.store.board)?;
            if let Some(requested_board_id) = args.board_id.as_deref()
                && preflight_board_id != requested_board_id
            {
                bail!(
                    "query label atom board mismatch: --board resolved to {preflight_board_id}, got --board-id {requested_board_id}"
                );
            }
            let reader = configured_active_reader_for_board(
                &args.store,
                LANCEDB_LABEL_ATOMS_STORE,
                &preflight_board_id,
            )?;
            let resolved_board_id = reader
                .resolved_board_id()
                .expect("board-scoped reader must retain the resolved board")
                .to_owned();
            if let Some(vector) = vector {
                let hits = reader.query_label_atoms_by_vector(&LabelAtomVectorQuery {
                    vector,
                    limit: args.limit,
                    board_id: Some(resolved_board_id),
                    embedding_model: args.embedding_model,
                    polarity: args.polarity,
                    include_vector: args.include_vector,
                })?;
                print_payload(vector_helper_query_label_atom_vectors_response(hits))
            } else {
                let hits = reader.query_label_atoms(&LabelAtomQuery {
                    text: args.text.unwrap_or_default(),
                    limit: args.limit,
                    board_id: Some(resolved_board_id),
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
            require_legacy_control(&args, LANCEDB_LABEL_ATOMS_STORE)?;
            let store = configured_store(&args)?;
            print_payload(vector_helper_status_response(
                rebuild_lancedb_label_atoms_with_store(&args.db, &args.board, &store)?,
            ))
        }
        Command::SyncLabelAtoms(args) => {
            require_legacy_control(&args, LANCEDB_LABEL_ATOMS_STORE)?;
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

    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_PROJECTION_STDIN_BYTES + 1)
        .read_to_end(&mut input)
        .context("failed to read vector projection request")?;
    let response = if input.len() as u64 > MAX_PROJECTION_STDIN_BYTES {
        vector_projection_invalid_request_response()
    } else {
        match decode_vector_projection_request(&input) {
            Ok(request) => match configured_projection_backend(&args)? {
                Some(backend) => backend.execute(&request),
                None => match request {
                    kanban_contract::VectorProjectionHelperRequest::Descriptor(request) => {
                        vector_projection_descriptor_response(request.request_id)
                    }
                    request => vector_projection_unavailable_response(&request),
                },
            },
            Err(_) => vector_projection_invalid_request_response(),
        }
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

fn configured_projection_backend(args: &ProjectionArgs) -> Result<Option<VectorProjectionBackend>> {
    if !args.db.is_file() {
        return Ok(None);
    }
    let Some(config) = kanban_local::resolved_vector_config(args.vector_config.as_deref())
        .with_context(|| "failed to read vector projection config")?
    else {
        return Ok(None);
    };
    let provider = Arc::new(provider_from_config(&config)?);
    VectorProjectionBackend::new(&args.db, provider)
        .map(Some)
        .map_err(|error| anyhow!(error))
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
    let Some(config) = resolved_config(args)? else {
        let mut status = LanceDbStore::connect(LanceDbConfig::degraded(
            kanban_local::vector_store_path(args.db.clone()),
        ))?
        .status();
        status.backend = "lancedb-label-atoms".to_owned();
        status.message =
            "LanceDB label atom helper is not configured; label atom retrieval degraded".to_owned();
        status.diagnostics.push("label_atom_helper".to_owned());
        return Ok(status);
    };
    let preflight_board_id = ActiveLanceProjectionReader::resolve_board(&args.db, &args.board)?;
    let base_status = VectorStoreStatus::new(
        "lancedb-label-atoms",
        true,
        format!(
            "LanceDB Projection v2 label atom helper enabled for Ollama endpoint {}, model {} ({} dimensions)",
            config.endpoint, config.model, config.dimensions,
        ),
    );
    active_reader_from_config_for_board_with_status(
        args,
        LANCEDB_LABEL_ATOMS_STORE,
        &config,
        &preflight_board_id,
        base_status,
    )
    .map(|(_, status)| status)
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
    let Some(config) = resolved_config(args)? else {
        let mut status = LanceDbStore::connect(LanceDbConfig::degraded(
            kanban_local::vector_store_path(args.db.clone()),
        ))?
        .status();
        status.message = "LanceDB vector helper unavailable; vector retrieval degraded".to_owned();
        status.diagnostics.push("vector_store_disabled".to_owned());
        return Ok(status);
    };
    let preflight_board_id = ActiveLanceProjectionReader::resolve_board(&args.db, &args.board)?;
    let base_status = VectorStoreStatus::new(
        "lancedb",
        true,
        format!(
            "LanceDB Projection v2 vector helper enabled for Ollama endpoint {}, model {} ({} dimensions)",
            config.endpoint, config.model, config.dimensions,
        ),
    );
    active_reader_from_config_for_board_with_status(
        args,
        LANCEDB_CHUNKS_STORE,
        &config,
        &preflight_board_id,
        base_status,
    )
    .map(|(_, status)| status)
}

fn configured_store(args: &StoreArgs) -> Result<LanceDbStore> {
    let config = required_config(args)?;
    let provider = Arc::new(provider_from_config(&config)?);
    LanceDbStore::connect(LanceDbConfig::new(
        kanban_local::vector_store_path(args.db.clone()),
        provider,
    ))
    .map_err(Into::into)
}

fn configured_active_reader_for_board(
    args: &StoreArgs,
    store_name: &str,
    expected_board_id: &str,
) -> Result<ActiveLanceProjectionReader> {
    let config = required_config(args)?;
    let provider = Arc::new(provider_from_config(&config)?);
    ActiveLanceProjectionReader::open_for_board(
        &args.db,
        store_name,
        &args.board,
        Some(expected_board_id),
        provider,
    )
    .map_err(Into::into)
}

fn active_reader_from_config_for_board_with_status(
    args: &StoreArgs,
    store_name: &str,
    config: &kanban_local::VectorConfig,
    expected_board_id: &str,
    base_status: VectorStoreStatus,
) -> Result<(ActiveLanceProjectionReader, VectorStoreStatus)> {
    let provider = Arc::new(provider_from_config(config)?);
    ActiveLanceProjectionReader::open_for_board_with_status(
        &args.db,
        store_name,
        &args.board,
        Some(expected_board_id),
        provider,
        base_status,
    )
    .map_err(Into::into)
}

fn required_config(args: &StoreArgs) -> Result<kanban_local::VectorConfig> {
    resolved_config(args)?.context(
        "vector helper requires a configured embedding provider; run `kanban vector configure` or pass --vector-config",
    )
}

fn require_legacy_control(args: &StoreArgs, store_name: &str) -> Result<()> {
    let conn = connect_file(&args.db)?;
    ensure_legacy_projection_control(&conn, store_name)?;
    Ok(())
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

fn print_payload(payload: impl Serialize) -> Result<()> {
    println!("{}", HelperEnvelope::new(payload)?.to_json()?);
    Ok(())
}
