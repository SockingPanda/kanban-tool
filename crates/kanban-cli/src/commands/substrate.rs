use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use kanban_context::ContextPolicy;
use kanban_sqlite::{
    EntityListOptions, MAX_SEARCH_LIMIT, MAX_TASK_LIST_LIMIT, OutboxListOptions,
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
use crate::output::print_or_json;

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
            print_or_json(json, &entities, || {
                entities
                    .iter()
                    .map(|entity| {
                        format!(
                            "{} [{}] {}:{}",
                            entity.uri, entity.kind, entity.source_table, entity.source_id
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        EntityCommand::Show { uri } => {
            let entity = get_entity(db_path, &uri)?;
            print_or_json(json, &entity, || {
                format!(
                    "{} [{}] {}:{} title={:?}",
                    entity.uri, entity.kind, entity.source_table, entity.source_id, entity.title
                )
            })?;
        }
    }
    Ok(())
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
            print_or_json(json, &jobs, || {
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
    Ok(())
}

pub(crate) fn handle_derived(command: DerivedCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        DerivedCommand::Status => {
            let statuses = derived_store_statuses(db_path)?;
            print_or_json(json, &statuses, || {
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
            let status = match graph_helper_json_classified::<kanban_graph::GraphStoreStatus>(
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
            print_or_json(json, &status, || {
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
                graph_helper_json::<Vec<kanban_entity::Relation>>(db_path, board, &helper_args)?;
            print_or_json(json, &neighbors, || {
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
            let status = graph_helper_json::<kanban_graph::GraphStoreStatus>(
                db_path,
                board,
                &["rebuild".to_owned()],
            )?;
            print_or_json(json, &status, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Sync => {
            let status = graph_helper_json::<kanban_graph::GraphStoreStatus>(
                db_path,
                board,
                &["sync".to_owned()],
            )?;
            print_or_json(json, &status, || {
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
            let rows = graph_helper_json::<Vec<kanban_graph::GraphQueryRow>>(
                db_path,
                board,
                &helper_args,
            )?;
            print_or_json(json, &rows, || {
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

fn graph_degraded_status(
    kind: HelperKind,
    error: &HelperRunError,
) -> kanban_graph::GraphStoreStatus {
    kanban_graph::GraphStoreStatus {
        backend: error.degraded_backend().to_owned(),
        enabled: false,
        message: helper_degraded_message(kind, error),
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
                run_helper_json::<serde_json::Value>(HelperKind::Vector, &helper_args)
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
            print_or_json(json, &config, || {
                format!(
                    "Configured vector provider {} model {} ({} dimensions) at {}",
                    config.provider, config.model, config.dimensions, config.endpoint
                )
            })?;
        }
        VectorCommand::Status(args) => {
            let status = match vector_helper_json_classified::<kanban_vector::VectorStoreStatus>(
                db_path,
                board,
                &["status".to_owned()],
                args.vector_config.as_deref(),
            ) {
                Ok(status) => status,
                Err(error) if error.is_status_degraded() => {
                    let mut status = kanban_vector::VectorStoreStatus::new(
                        error.degraded_backend(),
                        false,
                        helper_degraded_message(HelperKind::Vector, &error),
                    );
                    status
                        .diagnostics
                        .push(error.degraded_diagnostic().to_owned());
                    status
                }
                Err(error) => return Err(error.into()),
            };
            print_or_json(json, &status, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Rebuild(args) => {
            let status = vector_helper_json::<kanban_vector::VectorStoreStatus>(
                db_path,
                board,
                &["rebuild".to_owned()],
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &status, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Sync(args) => {
            let status = vector_helper_json::<kanban_vector::VectorStoreStatus>(
                db_path,
                board,
                &["sync".to_owned()],
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &status, || {
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
            let hits = vector_helper_json::<Vec<kanban_vector::VectorHit>>(
                db_path,
                board,
                &command_args,
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &hits, || {
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
            let values = vector_helper_json::<serde_json::Value>(
                db_path,
                board,
                &command_args,
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &values, || {
                let hits = values.as_array().cloned().unwrap_or_default();
                if hits.is_empty() {
                    "No label atom vector results".to_owned()
                } else {
                    hits.iter()
                        .map(|hit| {
                            let hit = hit.get("hit").unwrap_or(hit);
                            format!(
                                "{} label={} polarity={} distance={} {}",
                                hit.get("atom_id")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or(""),
                                hit.get("label_name")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or(""),
                                hit.get("polarity")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or(""),
                                hit.get("distance")
                                    .map(|value| value.to_string())
                                    .unwrap_or_default(),
                                hit.get("text")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("")
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
            print_or_json(json, &pack, || {
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
    kanban_sqlite::build_context_pack_with_vector_store(db_path, board, task_ref, policy, &store)
        .map_err(Into::into)
}
