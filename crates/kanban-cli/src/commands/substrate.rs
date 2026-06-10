use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use kanban_context::ContextPolicy;
use kanban_entity::EntityUri;
#[cfg(not(feature = "graph-oxigraph"))]
use kanban_graph::DisabledGraphStore;
#[cfg(feature = "graph-oxigraph")]
use kanban_graph::OxigraphStore;
use kanban_graph::RelationGraph;
use kanban_sqlite::{
    EntityListOptions, MAX_SEARCH_LIMIT, MAX_TASK_LIST_LIMIT, OutboxListOptions,
    derived_store_statuses, get_entity, list_entities, list_outbox, rebuild_graph_store,
    rebuild_vector_store, sync_graph_store, sync_vector_store, vector_store_status,
};

use crate::args::{
    ContextCommand, DerivedCommand, EntityCommand, GraphCommand, OutboxCommand, VectorCommand,
};
use crate::commands::common::{parse_predicate, validate_page_bounds};
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
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        GraphCommand::Status => {
            let status = kanban_sqlite::graph_store_status(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Neighbors(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let graph = open_graph_store(db_path)?;
            let uri = EntityUri::new(args.entity_uri)?;
            let predicate = args.predicate.as_deref().map(parse_predicate).transpose()?;
            let neighbors = graph.neighbors(&uri, predicate, args.limit)?;
            print_or_json(json, &neighbors, || {
                if neighbors.is_empty() {
                    "No graph neighbors (graph store disabled)".to_owned()
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
            let status = rebuild_graph_store(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Sync => {
            let status = sync_graph_store(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "graph backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        GraphCommand::Query(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let graph = open_graph_store(db_path)?;
            let rows = graph.query(&args.sparql, args.limit)?;
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

#[cfg(feature = "graph-oxigraph")]
pub(crate) fn open_graph_store(db_path: &Path) -> Result<OxigraphStore> {
    OxigraphStore::open(kanban_local::graph_store_path(db_path.to_path_buf())).map_err(Into::into)
}

#[cfg(not(feature = "graph-oxigraph"))]
pub(crate) fn open_graph_store(_db_path: &Path) -> Result<DisabledGraphStore> {
    Ok(DisabledGraphStore)
}

pub(crate) fn handle_vector(
    command: VectorCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        VectorCommand::Status => {
            let status = vector_store_status(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Rebuild => {
            let status = rebuild_vector_store(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        VectorCommand::Sync => {
            let status = sync_vector_store(db_path, board)?;
            print_or_json(json, &status, || {
                format!(
                    "vector backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
    }
    Ok(())
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
                bail!("max_items must be >= 1 because the subject item is mandatory");
            }
            let policy = ContextPolicy {
                lexical_limit: args.lexical_limit,
                graph_limit: args.graph_limit,
                vector_limit: args.vector_limit,
                max_items: args.max_items,
            };
            let pack = kanban_sqlite::build_context_pack(db_path, board, &args.task_ref, policy)?;
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
