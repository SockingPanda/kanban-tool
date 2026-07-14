use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use kanban_context::ContextPolicy;
use kanban_contract::{
    CliDerivedStatusOutput, CliDerivedStoreStatus, CliEntity, CliEntityListOutput,
    CliEntityShowOutput, CliOutboxItem, CliOutboxListOutput, GraphHelperNeighborsResponse,
    GraphHelperQueryResponse, GraphHelperRelation, GraphHelperStatusResponse,
    VectorHelperCheckProviderResponse, VectorHelperChunkHit, VectorHelperQueryChunksResponse,
    VectorHelperQueryLabelAtomsItem, VectorHelperQueryLabelAtomsResponse,
    VectorHelperStatusResponse,
    cli_helpers::{
        CliChunkRef, CliContextBuildOutput, CliGraphNeighborsOutput, CliGraphQueryBinding,
        CliGraphQueryOutput, CliGraphQueryRow, CliGraphRebuildOutput, CliGraphRelation,
        CliGraphRelationProvenance, CliGraphStatus, CliGraphStatusOutput, CliGraphSyncOutput,
        CliLabelAtomHit, CliLabelAtomVectorHit, CliVectorChunkHit, CliVectorConfig,
        CliVectorConfigureOutput, CliVectorLabelAtomHit, CliVectorQueryChunksOutput,
        CliVectorQueryLabelAtomsOutput, CliVectorRebuildOutput, CliVectorStatus,
        CliVectorStatusOutput, CliVectorSyncOutput,
    },
};
use kanban_core::KanbanError;
use kanban_sqlite::api::{
    EntityListOptions, EntityRecord, MAX_SEARCH_LIMIT, MAX_TASK_LIST_LIMIT, OutboxListOptions,
    derived_store_statuses, get_entity, list_entities, list_outbox,
};
use kanban_vector::SubprocessVectorStore;

use crate::args::{
    ContextCommand, DerivedCommand, EntityCommand, GraphCommand, OutboxCommand, VectorCommand,
    VectorConfigureArgs,
};
use crate::commands::common::{invalid_input, resolve_required_text_input, validate_page_bounds};
use crate::commands::helper::{
    HelperKind, HelperRunError, helper_degraded_message, resolve_helper, run_helper_json,
    run_helper_json_classified,
};
use crate::output::{print_contract_or_human, print_human};

pub(crate) fn handle_entity(command: EntityCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        EntityCommand::List(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let entities = list_entities(
                db_path,
                EntityListOptions {
                    kind: args.kind,
                    limit: args.limit,
                },
            )?;
            let output = CliEntityListOutput::new(
                entities.into_iter().map(cli_entity_from_record).collect(),
            );
            let human = output
                .data
                .iter()
                .map(|entity| {
                    format!(
                        "{} [{}] {}:{}",
                        entity.uri, entity.kind, entity.source_table, entity.source_id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            print_contract_or_human(json, &output, || human)?;
        }
        EntityCommand::Show { uri } => {
            let entity = get_entity(db_path, &uri)?;
            let output = CliEntityShowOutput::new(cli_entity_from_record(entity));
            let human = format!(
                "{} [{}] {}:{} title={:?}",
                output.data.uri,
                output.data.kind,
                output.data.source_table,
                output.data.source_id,
                output.data.title
            );
            print_contract_or_human(json, &output, || human)?;
        }
    }
    Ok(())
}

fn cli_entity_from_record(entity: EntityRecord) -> CliEntity {
    CliEntity {
        uri: entity.uri,
        kind: entity.kind,
        source_table: entity.source_table,
        source_id: entity.source_id,
        board_id: entity.board_id,
        task_id: entity.task_id,
        title: entity.title,
        summary: entity.summary,
        content_hash: entity.content_hash,
        created_at: entity.created_at,
        updated_at: entity.updated_at,
        archived_at: entity.archived_at,
    }
}

pub(crate) fn handle_outbox(command: OutboxCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        OutboxCommand::List(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let jobs = list_outbox(
                db_path,
                OutboxListOptions {
                    status: args.status,
                    limit: args.limit,
                },
            )?;
            if json {
                let output = CliOutboxListOutput::new(
                    jobs.into_iter()
                        .map(|job| -> Result<CliOutboxItem> {
                            Ok(CliOutboxItem {
                                id: job.id,
                                source_event_id: job.source_event_id,
                                target: job.target,
                                entity_uri: job.entity_uri,
                                action: job.action,
                                payload: serde_json::from_str(&job.payload_json).map_err(
                                    |error| {
                                        KanbanError::Storage(format!(
                                            "outbox item {} has invalid payload_json: {error}",
                                            job.id
                                        ))
                                    },
                                )?,
                                status: job.status,
                                attempts: job.attempts,
                                last_error: job.last_error,
                                created_at: job.created_at,
                                updated_at: job.updated_at,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
                print_contract_or_human(true, &output, String::new)?;
            } else {
                print_human(|| {
                    jobs.iter()
                        .map(|job| {
                            format!(
                                "#{} [{}] {} {} attempts={}",
                                job.id, job.status, job.target, job.entity_uri, job.attempts
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })?;
            }
        }
    }
    Ok(())
}

pub(crate) fn handle_derived(command: DerivedCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        DerivedCommand::Status => {
            let statuses = derived_store_statuses(db_path)?;
            if json {
                let output = CliDerivedStatusOutput::new(
                    statuses
                        .into_iter()
                        .map(|status| CliDerivedStoreStatus {
                            store_name: status.store_name,
                            schema_version: status.schema_version,
                            last_event_id: status.last_event_id,
                            dirty: status.dirty,
                            last_rebuild_at: status.last_rebuild_at,
                            last_sync_at: status.last_sync_at,
                            last_error: status.last_error,
                            updated_at: status.updated_at,
                        })
                        .collect(),
                );
                print_contract_or_human(true, &output, String::new)?;
            } else {
                print_human(|| {
                    statuses
                        .iter()
                        .map(|status| {
                            format!(
                                "{} schema={} last_event_id={} dirty={} last_error={:?}",
                                status.store_name,
                                status.schema_version,
                                status.last_event_id,
                                status.dirty,
                                status.last_error
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })?;
            }
        }
    }
    Ok(())
}

pub(crate) fn handle_graph(
    command: GraphCommand,
    db_path: &Path,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        GraphCommand::Status => {
            let status = match graph_helper_json_classified::<GraphHelperStatusResponse>(
                db_path,
                board,
                &["status".to_owned()],
            ) {
                Ok(status) => status,
                Err(error) if error.is_status_degraded() => {
                    graph_degraded_status(HelperKind::Graph, &error)
                }
                Err(error) => return Err(error.into()),
            };
            let output = CliGraphStatusOutput::new(cli_graph_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Neighbors(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let mut helper_args = vec![
                "neighbors".to_owned(),
                "--entity-uri".to_owned(),
                args.entity_uri,
                "--limit".to_owned(),
                args.limit.to_string(),
            ];
            if let Some(predicate) = args.predicate {
                helper_args.push("--predicate".to_owned());
                helper_args.push(predicate);
            }
            let neighbors =
                graph_helper_json::<GraphHelperNeighborsResponse>(db_path, board, &helper_args)?;
            let output =
                CliGraphNeighborsOutput::new(neighbors.iter().map(cli_graph_relation).collect());
            print_contract_or_human(json, &output, || {
                if neighbors.is_empty() {
                    "No graph neighbors".to_owned()
                } else {
                    neighbors
                        .iter()
                        .map(|relation| {
                            format!(
                                "{} --{}--> {}",
                                relation.subject_uri, relation.predicate, relation.object_uri
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })?;
        }
        GraphCommand::Rebuild => {
            let status = graph_helper_json::<GraphHelperStatusResponse>(
                db_path,
                board,
                &["rebuild".to_owned()],
            )?;
            let output = CliGraphRebuildOutput::new(cli_graph_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Sync => {
            let status = graph_helper_json::<GraphHelperStatusResponse>(
                db_path,
                board,
                &["sync".to_owned()],
            )?;
            let output = CliGraphSyncOutput::new(cli_graph_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Query(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let sparql = resolve_required_text_input(
                args.sparql,
                args.sparql_file,
                "SPARQL",
                "--sparql-file",
                "SPARQL",
            )?;
            let helper_args = vec![
                "query".to_owned(),
                "--sparql".to_owned(),
                sparql,
                "--limit".to_owned(),
                args.limit.to_string(),
            ];
            let rows = graph_helper_json::<GraphHelperQueryResponse>(db_path, board, &helper_args)?;
            let output = CliGraphQueryOutput::new(rows.iter().map(cli_graph_query_row).collect());
            print_contract_or_human(json, &output, || {
                if rows.is_empty() {
                    "No graph query results".to_owned()
                } else {
                    rows.iter()
                        .map(|row| {
                            row.bindings
                                .iter()
                                .map(|binding| format!("{}={}", binding.name, binding.value))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })?;
        }
    }
    Ok(())
}

fn graph_helper_json<T>(db_path: &Path, board: &str, command_args: &[String]) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(db_path.display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    run_helper_json(HelperKind::Graph, &args)
}

fn graph_helper_json_classified<T>(
    db_path: &Path,
    board: &str,
    command_args: &[String],
) -> std::result::Result<T, HelperRunError>
where
    T: serde::de::DeserializeOwned,
{
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(db_path.display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    run_helper_json_classified(HelperKind::Graph, &args)
}

fn graph_degraded_status(kind: HelperKind, error: &HelperRunError) -> GraphHelperStatusResponse {
    GraphHelperStatusResponse {
        backend: error.degraded_backend().to_owned(),
        enabled: false,
        message: helper_degraded_message(kind, error),
    }
}

fn cli_graph_status(status: &GraphHelperStatusResponse) -> CliGraphStatus {
    CliGraphStatus {
        backend: status.backend.clone(),
        enabled: status.enabled,
        message: status.message.clone(),
    }
}

fn cli_graph_relation(relation: &GraphHelperRelation) -> CliGraphRelation {
    CliGraphRelation {
        subject_uri: relation.subject_uri.clone(),
        predicate: relation.predicate.clone(),
        object_uri: relation.object_uri.clone(),
        graph_uri: relation.graph_uri.clone(),
        provenance: CliGraphRelationProvenance {
            source_table: relation.provenance.source_table.clone(),
            source_id: relation.provenance.source_id.clone(),
            source_event_id: relation.provenance.source_event_id,
            authoritative_store: relation.provenance.authoritative_store.clone(),
        },
        metadata: relation.metadata.clone(),
        created_at: relation.created_at,
        updated_at: relation.updated_at,
    }
}

fn cli_graph_query_row(row: &kanban_contract::GraphHelperQueryRow) -> CliGraphQueryRow {
    CliGraphQueryRow {
        bindings: row
            .bindings
            .iter()
            .map(|binding| CliGraphQueryBinding {
                name: binding.name.clone(),
                value: binding.value.clone(),
            })
            .collect(),
    }
}

pub(crate) fn handle_vector(
    command: VectorCommand,
    db_path: &Path,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        VectorCommand::Configure(args) => {
            let config = vector_config_from_args(&args)?;
            if !args.skip_check {
                let helper_args = vec![
                    "check-provider".to_owned(),
                    "--endpoint".to_owned(),
                    config.endpoint.clone(),
                    "--model".to_owned(),
                    config.model.clone(),
                    "--dimensions".to_owned(),
                    config.dimensions.to_string(),
                ];
                run_helper_json::<VectorHelperCheckProviderResponse>(
                    HelperKind::Vector,
                    &helper_args,
                )
                .with_context(|| "Ollama embedding check failed; config was not written")?;
            }
            match args.vector_config.as_deref() {
                Some(path) => kanban_local::write_vector_config_at(path, config.clone())
                    .with_context(|| "failed to write vector config")?,
                None => {
                    kanban_local::write_vector_config(config.clone())
                        .with_context(|| "failed to write global vector config")?;
                }
            }
            let output = CliVectorConfigureOutput::new(cli_vector_config(&config));
            print_contract_or_human(json, &output, || {
                format!(
                    "Configured vector provider {} model {} ({} dimensions) at {}",
                    config.provider, config.model, config.dimensions, config.endpoint
                )
            })?;
        }
        VectorCommand::Status(args) => {
            let status = match vector_helper_json_classified::<VectorHelperStatusResponse>(
                db_path,
                board,
                &["status".to_owned()],
                args.vector_config.as_deref(),
            ) {
                Ok(status) => status,
                Err(error) if error.is_status_degraded() => VectorHelperStatusResponse {
                    backend: error.degraded_backend().to_owned(),
                    enabled: false,
                    message: helper_degraded_message(HelperKind::Vector, &error),
                    diagnostics: vec![error.degraded_diagnostic().to_owned()],
                    dirty: None,
                    board_dirty: None,
                    generation: None,
                },
                Err(error) => return Err(error.into()),
            };
            let output = CliVectorStatusOutput::new(cli_vector_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Rebuild(args) => {
            let status = vector_helper_json::<VectorHelperStatusResponse>(
                db_path,
                board,
                &["rebuild".to_owned()],
                args.vector_config.as_deref(),
            )?;
            let output = CliVectorRebuildOutput::new(cli_vector_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Sync(args) => {
            let status = vector_helper_json::<VectorHelperStatusResponse>(
                db_path,
                board,
                &["sync".to_owned()],
                args.vector_config.as_deref(),
            )?;
            let output = CliVectorSyncOutput::new(cli_vector_status(&status));
            print_contract_or_human(json, &output, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::QueryChunks(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let command_args = vec![
                "query-chunks".to_owned(),
                "--text".to_owned(),
                args.text,
                "--limit".to_owned(),
                args.limit.to_string(),
            ];
            let hits = vector_helper_json::<VectorHelperQueryChunksResponse>(
                db_path,
                board,
                &command_args,
                args.vector_config.as_deref(),
            )?;
            let output =
                CliVectorQueryChunksOutput::new(hits.iter().map(cli_vector_chunk_hit).collect());
            print_contract_or_human(json, &output, || {
                if hits.is_empty() {
                    "No vector chunk results".to_owned()
                } else {
                    hits.iter()
                        .map(|hit| {
                            format!(
                                "{} score={} {}",
                                hit.chunk.entity_uri,
                                hit.score,
                                hit.summary.as_deref().unwrap_or("")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })?;
        }
        VectorCommand::QueryLabelAtoms(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let mut command_args = vec!["query-label-atoms".to_owned()];
            if args.text.is_some() || args.text_file.is_some() {
                let text = resolve_required_text_input(
                    args.text,
                    args.text_file,
                    "TEXT",
                    "--text-file",
                    "TEXT",
                )?;
                command_args.push("--text".to_owned());
                command_args.push(text);
            } else {
                let vector_json = resolve_required_text_input(
                    args.vector_json,
                    args.vector_json_file,
                    "--vector-json",
                    "--vector-json-file",
                    "--vector-json",
                )?;
                command_args.push("--vector-json".to_owned());
                command_args.push(vector_json);
            }
            command_args.push("--limit".to_owned());
            command_args.push(args.limit.to_string());
            if let Some(board_id) = args.board_id {
                command_args.push("--board-id".to_owned());
                command_args.push(board_id);
            }
            if let Some(embedding_model) = args.embedding_model {
                command_args.push("--embedding-model".to_owned());
                command_args.push(embedding_model);
            }
            if let Some(polarity) = args.polarity {
                command_args.push("--polarity".to_owned());
                command_args.push(polarity);
            }
            if args.include_vector {
                command_args.push("--include-vector".to_owned());
            }
            let values = vector_helper_json::<VectorHelperQueryLabelAtomsResponse>(
                db_path,
                board,
                &command_args,
                args.vector_config.as_deref(),
            )?;
            let output = CliVectorQueryLabelAtomsOutput::new(
                values.into_iter().map(cli_vector_label_atom_hit).collect(),
            );
            print_contract_or_human(json, &output, || {
                if output.data.is_empty() {
                    "No label atom vector results".to_owned()
                } else {
                    output
                        .data
                        .iter()
                        .map(|hit| {
                            let hit = match hit {
                                CliVectorLabelAtomHit::Hit(hit) => hit,
                                CliVectorLabelAtomHit::WithVector(hit) => &hit.hit,
                            };
                            format!(
                                "{} label={} polarity={} distance={} {}",
                                hit.atom_id, hit.label_name, hit.polarity, hit.distance, hit.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })?;
        }
    }
    Ok(())
}

fn vector_helper_json<T>(
    db_path: &Path,
    board: &str,
    command_args: &[String],
    vector_config_path: Option<&Path>,
) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(db_path.display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    if let Some(path) = vector_config_path {
        args.push("--vector-config".to_owned());
        args.push(path.display().to_string());
    }
    run_helper_json(HelperKind::Vector, &args)
}

fn vector_helper_json_classified<T>(
    db_path: &Path,
    board: &str,
    command_args: &[String],
    vector_config_path: Option<&Path>,
) -> std::result::Result<T, HelperRunError>
where
    T: serde::de::DeserializeOwned,
{
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(db_path.display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    if let Some(path) = vector_config_path {
        args.push("--vector-config".to_owned());
        args.push(path.display().to_string());
    }
    run_helper_json_classified(HelperKind::Vector, &args)
}

pub(crate) fn handle_context(
    command: ContextCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        ContextCommand::Build(args) => {
            validate_page_bounds(args.lexical_limit, MAX_SEARCH_LIMIT, 0)?;
            validate_page_bounds(args.graph_limit, MAX_TASK_LIST_LIMIT, 0)?;
            validate_page_bounds(args.vector_limit, MAX_TASK_LIST_LIMIT, 0)?;
            validate_page_bounds(args.max_items, MAX_TASK_LIST_LIMIT, 0)?;
            if args.max_items == 0 {
                return Err(invalid_input(
                    "max_items must be >= 1 because the subject item is mandatory",
                ));
            }
            let policy = ContextPolicy {
                lexical_limit: args.lexical_limit,
                graph_limit: args.graph_limit,
                vector_limit: args.vector_limit,
                max_items: args.max_items,
            };
            let pack = build_configured_context_pack(
                db_path,
                board,
                &args.task_ref,
                policy,
                args.vector_config.as_deref(),
            )?;
            let output = CliContextBuildOutput::new(cli_context_pack(&pack));
            print_contract_or_human(json, &output, || {
                format!(
                    "context subject={} items={} degraded={}",
                    pack.subject,
                    pack.items.len(),
                    pack.degraded.join(",")
                )
            })?;
        }
    }
    Ok(())
}

fn vector_config_from_args(args: &VectorConfigureArgs) -> Result<kanban_local::VectorConfig> {
    if args.provider != "ollama" {
        return Err(invalid_input(format!(
            "unsupported vector provider: {}",
            args.provider
        )));
    }
    if args.dimensions == 0 {
        return Err(invalid_input("dimensions must be greater than zero"));
    }
    Ok(kanban_local::VectorConfig {
        provider: args.provider.clone(),
        endpoint: args.endpoint.clone(),
        model: args.model.clone(),
        dimensions: args.dimensions,
    })
}

fn cli_vector_config(config: &kanban_local::VectorConfig) -> CliVectorConfig {
    CliVectorConfig {
        provider: config.provider.clone(),
        endpoint: config.endpoint.clone(),
        model: config.model.clone(),
        dimensions: config.dimensions,
    }
}

fn cli_vector_status(status: &VectorHelperStatusResponse) -> CliVectorStatus {
    CliVectorStatus {
        backend: status.backend.clone(),
        enabled: status.enabled,
        message: status.message.clone(),
        diagnostics: status.diagnostics.clone(),
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    }
}

fn cli_vector_chunk_hit(hit: &VectorHelperChunkHit) -> CliVectorChunkHit {
    CliVectorChunkHit {
        chunk: CliChunkRef {
            uri: hit.chunk.uri.clone(),
            entity_uri: hit.chunk.entity_uri.clone(),
            ordinal: hit.chunk.ordinal,
            content_hash: hit.chunk.content_hash.clone(),
        },
        score: hit.score,
        text: hit.text.clone(),
        summary: hit.summary.clone(),
    }
}

fn cli_label_atom_hit(hit: kanban_contract::VectorHelperLabelAtomHit) -> CliLabelAtomHit {
    CliLabelAtomHit {
        atom_id: hit.atom_id,
        label_id: hit.label_id,
        label_name: hit.label_name,
        board_id: hit.board_id,
        polarity: hit.polarity,
        kind: hit.kind,
        text: hit.text,
        ordinal: hit.ordinal,
        content_hash: hit.content_hash,
        embedding_model: hit.embedding_model,
        distance: hit.distance,
    }
}

fn cli_vector_label_atom_hit(hit: VectorHelperQueryLabelAtomsItem) -> CliVectorLabelAtomHit {
    match hit {
        VectorHelperQueryLabelAtomsItem::Hit(hit) => {
            CliVectorLabelAtomHit::Hit(cli_label_atom_hit(hit))
        }
        VectorHelperQueryLabelAtomsItem::WithVector(hit) => {
            CliVectorLabelAtomHit::WithVector(CliLabelAtomVectorHit {
                hit: cli_label_atom_hit(hit.hit),
                vector: hit.vector,
            })
        }
    }
}

fn cli_context_pack(pack: &kanban_context::ContextPack) -> kanban_contract::ContextPack {
    kanban_contract::ContextPack {
        subject: pack.subject.to_string(),
        policy: kanban_contract::ContextPolicy {
            lexical_limit: pack.policy.lexical_limit,
            graph_limit: pack.policy.graph_limit,
            vector_limit: pack.policy.vector_limit,
            max_items: pack.policy.max_items,
        },
        items: pack
            .items
            .iter()
            .map(|item| kanban_contract::ContextItem {
                entity_uri: item.entity_uri.to_string(),
                source: item.source.clone(),
                provenance: item.provenance.clone(),
                score: item.score,
                title: item.title.clone(),
                snippet: item.snippet.clone(),
            })
            .collect(),
        degraded: pack.degraded.clone(),
        diagnostics: pack
            .diagnostics
            .iter()
            .map(|diagnostic| kanban_contract::ContextDiagnostic {
                source: diagnostic.source.clone(),
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
            })
            .collect(),
    }
}

fn build_configured_context_pack(
    db_path: &PathBuf,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
    vector_config_path: Option<&Path>,
) -> Result<kanban_context::ContextPack> {
    let mut store = SubprocessVectorStore::new(
        resolve_helper(HelperKind::Vector),
        db_path.clone(),
        board.to_owned(),
        vector_config_path.map(Path::to_path_buf),
    );
    if let Some(config) = kanban_local::resolved_vector_config(vector_config_path)? {
        store = store.with_embedding_model(config.model);
    }
    kanban_sqlite::api::provider::build_context_pack_with_vector_store(
        db_path, board, task_ref, policy, &store,
    )
    .map_err(Into::into)
}
