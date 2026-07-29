use crate::db::connect_file;

use super::{
    MAX_SEARCH_LIMIT, board_id, current_last_event_id, search_lag, sqlite_like_literal, storage,
    task_ref_filter, validate_page_bounds,
};
#[cfg(feature = "tantivy-backend")]
use super::{
    ensure_legacy_projection_control, maintenance_status, mark_derived_store_dirty,
    mark_derived_store_failure, mark_derived_store_success,
    tantivy_projection::TantivyProjectionStore, validate_physical_active_artifact_with,
};

#[cfg(feature = "tantivy-backend")]
use std::collections::HashSet;
use std::path::Path;
#[cfg(feature = "tantivy-backend")]
use std::path::PathBuf;

use kanban_core::Result;
#[cfg(feature = "tantivy-backend")]
use kanban_core::{Clock, KanbanError, SystemClock, TaskStatus};
#[cfg(feature = "tantivy-backend")]
use kanban_indexer::TANTIVY_TASKS_STORE;

#[cfg(feature = "tantivy-backend")]
use kanban_search::TaskSearchDocument;
use kanban_search::{SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults};

#[cfg(feature = "tantivy-backend")]
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite::{params_from_iter, types::Value};

use serde::{Deserialize, Serialize};

#[cfg(feature = "tantivy-backend")]
const SEARCH_TASKS_STATE_SCHEMA_VERSION: i64 = 1;
#[cfg(feature = "tantivy-backend")]
const SEARCH_TASKS_STATE_KEY_PREFIX: &str = "search.tasks.state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexState {
    pub schema_version: i64,
    pub index_version: String,
    pub backend: String,
    pub index_name: String,
    pub board_id: String,
    pub last_event_id: Option<i64>,
    pub dirty: bool,
    pub updated_at: i64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct SearchProjectionContext {
    database_instance_id: Option<String>,
    protocol_version: Option<i64>,
    generation: Option<String>,
    fallback_reason: Option<String>,
}

pub fn search_tasks(path: impl AsRef<Path>, query: SearchQuery) -> Result<SearchResults> {
    validate_page_bounds(query.limit, MAX_SEARCH_LIMIT, query.offset)?;
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let conn = connect_file(path_ref)?;
        let board_id = board_id(&conn, &query.board)?;
        let last_event_id = current_last_event_id(&conn, &board_id)?;
        if let Some(store) = maintenance_status(path_ref)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE && store.control_plane == "v2")
        {
            let query_requires_sqlite = tantivy_sqlite_fallback_required(&query);
            let fallback_reason = store.fallback_reason.clone().or_else(|| {
                if store.active_generation.is_none() {
                    Some("active_generation_missing".to_owned())
                } else if store.active_provider.as_deref()
                    != Some(super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER)
                    || store.active_provider_fingerprint.as_deref()
                        != Some(super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER_FINGERPRINT)
                {
                    Some("provider_mismatch".to_owned())
                } else {
                    None
                }
            });
            let unavailable = fallback_reason.is_some();
            let projection_context = SearchProjectionContext {
                database_instance_id: Some(store.database_instance_id.clone()),
                protocol_version: Some(store.protocol_version),
                generation: store.active_generation.clone(),
                fallback_reason: fallback_reason
                    .clone()
                    .or_else(|| query_requires_sqlite.then(|| "query_requires_sqlite".to_owned())),
            };
            if query_requires_sqlite || unavailable {
                return sqlite_search_tasks(
                    path_ref,
                    query,
                    unavailable,
                    if unavailable { None } else { last_event_id },
                    projection_context,
                );
            }
            let generation = store
                .active_generation
                .as_deref()
                .expect("checked active generation");
            let backend = TantivyProjectionStore::new(path_ref)?;
            let expected = match validate_physical_active_artifact_with(
                path_ref,
                TANTIVY_TASKS_STORE,
                &backend,
            ) {
                Ok(Some(expected)) => expected,
                _ => {
                    return sqlite_search_tasks(
                        path_ref,
                        query,
                        true,
                        None,
                        SearchProjectionContext {
                            fallback_reason: Some("physical_generation_unavailable".to_owned()),
                            ..projection_context
                        },
                    );
                }
            };
            let mut scoped_query = query.clone();
            scoped_query.board.clone_from(&board_id);
            debug_assert_eq!(expected.manifest.generation, generation);
            match backend.search_active(&expected, &scoped_query) {
                Ok((hits, mut meta)) => {
                    meta.last_event_id = last_event_id;
                    meta.index_lag_events = Some(0);
                    return Ok(SearchResults { hits, meta });
                }
                Err(_) => {
                    return sqlite_search_tasks(
                        path_ref,
                        query,
                        true,
                        None,
                        SearchProjectionContext {
                            fallback_reason: Some("physical_generation_unavailable".to_owned()),
                            ..projection_context
                        },
                    );
                }
            }
        }
        let state = read_search_index_state(&conn, &board_id)?;
        let indexed_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        let index_path = task_index_path(path_ref);
        if kanban_search::tantivy_backend::task_index_exists(&index_path) {
            let metadata =
                match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                    Ok(metadata) => metadata,
                    Err(err) if err.is_fallback_eligible() => {
                        return sqlite_search_tasks(
                            path_ref,
                            query,
                            true,
                            indexed_last_event_id,
                            SearchProjectionContext {
                                fallback_reason: Some("legacy_index_unavailable".to_owned()),
                                ..Default::default()
                            },
                        );
                    }
                    Err(err) => return Err(search_storage(err)),
                };
            let contract = search_index_contract(indexed_last_event_id, &metadata);
            let validated_indexed_last_event_id = contract.indexed_last_event_id;
            if contract.mismatch {
                return sqlite_search_tasks(
                    path_ref,
                    query,
                    true,
                    validated_indexed_last_event_id,
                    SearchProjectionContext {
                        fallback_reason: Some("legacy_index_metadata_mismatch".to_owned()),
                        ..Default::default()
                    },
                );
            }
            if search_index_ahead(last_event_id, validated_indexed_last_event_id) {
                return sqlite_search_tasks(
                    path_ref,
                    query,
                    true,
                    validated_indexed_last_event_id,
                    SearchProjectionContext {
                        fallback_reason: Some("legacy_index_ahead".to_owned()),
                        ..Default::default()
                    },
                );
            }
            if tantivy_sqlite_fallback_required(&query) {
                return sqlite_search_tasks(
                    path_ref,
                    query,
                    true,
                    validated_indexed_last_event_id,
                    SearchProjectionContext {
                        fallback_reason: Some("query_requires_sqlite".to_owned()),
                        ..Default::default()
                    },
                );
            }
            match kanban_search::tantivy_backend::search_task_index(
                &index_path,
                &board_id,
                &query,
                last_event_id,
            ) {
                Ok(results) => {
                    let indexed = search_index_contract(indexed_last_event_id, &metadata)
                        .indexed_last_event_id;
                    let lag = search_lag(last_event_id, indexed);
                    if search_index_ahead(last_event_id, indexed) {
                        return sqlite_search_tasks(
                            path_ref,
                            query,
                            true,
                            indexed,
                            SearchProjectionContext {
                                fallback_reason: Some("legacy_index_ahead".to_owned()),
                                ..Default::default()
                            },
                        );
                    }
                    if state.as_ref().is_some_and(|state| state.dirty) || lag > 0 || results.1.stale
                    {
                        return sqlite_search_tasks(
                            path_ref,
                            query,
                            true,
                            indexed,
                            SearchProjectionContext {
                                fallback_reason: Some("legacy_index_stale".to_owned()),
                                ..Default::default()
                            },
                        );
                    }
                    return Ok(SearchResults {
                        hits: results.0,
                        meta: SearchMeta {
                            last_event_id: indexed,
                            index_lag_events: Some(lag),
                            stale: false,
                            ..results.1
                        },
                    });
                }
                Err(err) if err.is_fallback_eligible() => {
                    return sqlite_search_tasks(
                        path_ref,
                        query,
                        true,
                        validated_indexed_last_event_id,
                        SearchProjectionContext {
                            fallback_reason: Some("legacy_index_unavailable".to_owned()),
                            ..Default::default()
                        },
                    );
                }
                Err(err) => return Err(search_storage(err)),
            }
        }
    }
    sqlite_search_tasks(path, query, false, None, SearchProjectionContext::default())
}

fn sqlite_search_tasks(
    path: impl AsRef<Path>,
    query: SearchQuery,
    stale: bool,
    indexed_last_event_id: Option<i64>,
    projection: SearchProjectionContext,
) -> Result<SearchResults> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, &query.board)?;
    let (where_sql, mut params) = search_task_where(&board_id, &query);
    let exact_ref_search = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| task_ref_filter(value, "t.").is_some());
    let needle = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|_| !exact_ref_search)
        .map(|value| format!("%{}%", sqlite_like_literal(&value.to_lowercase())));
    let (score_sql, snippet_sql, outer_filter, mut search_params) = if let Some(needle) =
        needle.as_deref()
    {
        (
            "CASE WHEN lower(t.title) LIKE ? ESCAPE '\\' THEN 100.0 ELSE 0.0 END \
                 + CASE WHEN lower(COALESCE(t.description, '')) LIKE ? ESCAPE '\\' THEN 60.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_comments c WHERE c.task_id=t.id AND lower(c.body) LIKE ? ESCAPE '\\') THEN 40.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_runs r WHERE r.task_id=t.id AND (lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' OR lower(COALESCE(r.error, '')) LIKE ? ESCAPE '\\')) THEN 30.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_events e WHERE e.task_id=t.id AND (lower(e.kind) LIKE ? ESCAPE '\\' OR lower(e.payload_json) LIKE ? ESCAPE '\\')) THEN 20.0 ELSE 0.0 END",
            "COALESCE(\
                    CASE WHEN lower(t.title) LIKE ? ESCAPE '\\' THEN t.title END,\
                    CASE WHEN lower(COALESCE(t.description, '')) LIKE ? ESCAPE '\\' THEN t.description END,\
                    (SELECT c.body FROM task_comments c WHERE c.task_id=t.id AND lower(c.body) LIKE ? ESCAPE '\\' ORDER BY c.created_at ASC, c.id ASC LIMIT 1),\
                    (SELECT CASE WHEN lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' THEN r.summary ELSE r.error END FROM task_runs r WHERE r.task_id=t.id AND (lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' OR lower(COALESCE(r.error, '')) LIKE ? ESCAPE '\\') ORDER BY r.started_at DESC, r.id ASC LIMIT 1),\
                    (SELECT e.kind || ' ' || e.payload_json FROM task_events e WHERE e.task_id=t.id AND (lower(e.kind) LIKE ? ESCAPE '\\' OR lower(e.payload_json) LIKE ? ESCAPE '\\') ORDER BY e.id ASC LIMIT 1)\
                )",
            "WHERE score > 0.0",
            vec![
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
            ],
        )
    } else if exact_ref_search {
        ("120.0", "t.id", "WHERE score > 0.0", Vec::new())
    } else {
        ("0.0", "NULL", "", Vec::new())
    };
    search_params.append(&mut params);
    search_params.push(Value::Integer(
        query.limit.try_into().expect("validated limit"),
    ));
    search_params.push(Value::Integer(
        query.offset.try_into().expect("validated offset"),
    ));
    let sql = format!(
        "SELECT task_id, seq, score, snippet FROM (\
             SELECT t.id AS task_id, t.seq AS seq, ({score_sql}) AS score, {snippet_sql} AS snippet, t.updated_at AS updated_at \
             FROM tasks t {where_sql}\
         ) {outer_filter} ORDER BY score DESC, updated_at DESC, seq ASC LIMIT ? OFFSET ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(search_params.iter()), |row| {
            Ok(SearchHit {
                task_id: row.get(0)?,
                seq: row.get(1)?,
                score: row.get(2)?,
                snippet: row.get(3)?,
            })
        })
        .map_err(storage)?;
    let hits = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    Ok(SearchResults {
        hits,
        meta: sqlite_search_meta(
            last_event_id,
            stale,
            indexed_last_event_id,
            &board_id,
            projection,
        ),
    })
}

pub fn search_index_status(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    #[cfg(feature = "tantivy-backend")]
    {
        if let Some(store) = maintenance_status(path.as_ref())?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE && store.control_plane == "v2")
        {
            let generation = store.active_generation.clone();
            let provider_matches = store.active_provider.as_deref()
                == Some(super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER)
                && store.active_provider_fingerprint.as_deref()
                    == Some(super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER_FINGERPRINT);
            let physical_ready = if generation.is_some()
                && provider_matches
                && store.fallback_reason.is_none()
            {
                let backend = TantivyProjectionStore::new(path.as_ref())?;
                validate_physical_active_artifact_with(path.as_ref(), TANTIVY_TASKS_STORE, &backend)
                    .is_ok_and(|evidence| evidence.is_some())
            } else {
                false
            };
            let stale = store.fallback_reason.is_some() || !physical_ready;
            let fallback_reason = if stale {
                Some(
                    store
                        .fallback_reason
                        .clone()
                        .unwrap_or_else(|| "physical_generation_unavailable".to_owned()),
                )
            } else {
                None
            };
            return Ok(SearchIndexStatus {
                backend: if stale { "sqlite" } else { "tantivy" }.to_owned(),
                derived_index: true,
                stale,
                database_instance_id: Some(store.database_instance_id.clone()),
                protocol_version: Some(store.protocol_version),
                generation: generation.clone(),
                resolved_board_id: board_id.clone(),
                fallback_reason,
                index_version: generation
                    .as_ref()
                    .map(|_| kanban_search::tantivy_backend::PROJECTION_INDEX_VERSION.to_owned()),
                last_event_id: if stale { None } else { last_event_id },
                index_lag_events: if stale { None } else { Some(0) },
                message: if stale {
                    format!(
                        "Projection v2 Tantivy search is unavailable ({}); SQLite fallback search is active",
                        store
                            .fallback_reason
                            .as_deref()
                            .unwrap_or("physical_generation_unavailable")
                    )
                } else {
                    format!(
                        "Projection v2 Tantivy generation {} is active",
                        generation.as_deref().expect("physical generation checked")
                    )
                },
            });
        }
        let index_path = task_index_path(path.as_ref());
        let state = read_search_index_state(&conn, &board_id)?;
        let indexed_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        if kanban_search::tantivy_backend::task_index_exists(&index_path) {
            let metadata =
                match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                    Ok(metadata) => metadata,
                    Err(err) if err.is_fallback_eligible() => {
                        return Ok(degraded_search_index_status(
                            last_event_id,
                            indexed_last_event_id,
                            &index_path,
                            &err,
                            &board_id,
                        ));
                    }
                    Err(err) => return Err(search_storage(err)),
                };
            let contract = search_index_contract(indexed_last_event_id, &metadata);
            let indexed = contract.indexed_last_event_id;
            let lag = search_lag(last_event_id, indexed);
            let dirty = state.as_ref().is_some_and(|state| state.dirty);
            if contract.mismatch {
                return Ok(mismatched_search_index_status(
                    last_event_id,
                    indexed,
                    &index_path,
                    Some(metadata.index_version),
                    indexed_last_event_id,
                    metadata.last_event_id,
                    &board_id,
                ));
            }
            if search_index_ahead(last_event_id, indexed) {
                return Ok(search_index_ahead_status(
                    last_event_id,
                    indexed,
                    &index_path,
                    Some(metadata.index_version),
                    &board_id,
                ));
            }
            return Ok(SearchIndexStatus {
                backend: "tantivy".to_owned(),
                derived_index: true,
                stale: dirty || lag > 0,
                database_instance_id: None,
                protocol_version: None,
                generation: None,
                resolved_board_id: board_id.clone(),
                fallback_reason: (dirty || lag > 0).then(|| "legacy_index_stale".to_owned()),
                index_version: Some(metadata.index_version),
                last_event_id: indexed,
                index_lag_events: Some(lag),
                message: state
                    .and_then(|state| state.message)
                    .unwrap_or_else(|| format!("Tantivy task index at {}", index_path.display())),
            });
        }
    }
    Ok(SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: false,
        stale: false,
        database_instance_id: None,
        protocol_version: None,
        generation: None,
        resolved_board_id: board_id,
        fallback_reason: None,
        index_version: None,
        last_event_id,
        index_lag_events: Some(0),
        message: "SQLite fallback search is active; no derived index exists yet".to_owned(),
    })
}

pub fn rebuild_search_index(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let _write_guard =
            crate::db::acquire_derived_store_write_guard(path_ref, TANTIVY_TASKS_STORE)?;
        let conn = connect_file(path_ref)?;
        ensure_legacy_projection_control(&conn, TANTIVY_TASKS_STORE)?;
        let board_id = board_id(&conn, board)?;
        let last_event_id = current_last_event_id(&conn, &board_id)?;
        let documents = task_search_documents(&conn, &board_id)?;
        let index_path = task_index_path(path_ref);
        let metadata = match kanban_search::tantivy_backend::rebuild_task_index(
            &index_path,
            &board_id,
            last_event_id,
            &documents,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                mark_derived_store_failure(
                    &conn,
                    TANTIVY_TASKS_STORE,
                    &board_id,
                    &error.to_string(),
                    SystemClock.now_ms(),
                )?;
                return Err(search_storage(error));
            }
        };
        let now = SystemClock.now_ms();
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                metadata.last_event_id,
                false,
                Some(format!(
                    "Rebuilt Tantivy task index at {}",
                    index_path.display()
                )),
                now,
            ),
        )?;
        mark_derived_store_success(
            &conn,
            TANTIVY_TASKS_STORE,
            &board_id,
            metadata.last_event_id,
            true,
            now,
        )?;
        Ok(SearchIndexStatus {
            backend: "tantivy".to_owned(),
            derived_index: true,
            stale: false,
            database_instance_id: None,
            protocol_version: None,
            generation: None,
            resolved_board_id: board_id,
            fallback_reason: None,
            index_version: Some(metadata.index_version),
            last_event_id: metadata.last_event_id,
            index_lag_events: Some(0),
            message: format!("Rebuilt Tantivy task index at {}", index_path.display()),
        })
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        search_index_status(path, board)
    }
}

pub fn sync_search_index(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let _write_guard =
            crate::db::acquire_derived_store_write_guard(path_ref, TANTIVY_TASKS_STORE)?;
        let conn = connect_file(path_ref)?;
        ensure_legacy_projection_control(&conn, TANTIVY_TASKS_STORE)?;
        let board_id = board_id(&conn, board)?;
        let index_path = task_index_path(path_ref);
        if !kanban_search::tantivy_backend::task_index_exists(&index_path) {
            drop(_write_guard);
            return rebuild_search_index(path_ref, board);
        }
        let metadata =
            match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                Ok(metadata) => metadata,
                Err(error) => {
                    mark_derived_store_failure(
                        &conn,
                        TANTIVY_TASKS_STORE,
                        &board_id,
                        &error.to_string(),
                        SystemClock.now_ms(),
                    )?;
                    return Err(search_storage(error));
                }
            };
        let state = read_search_index_state(&conn, &board_id)?;
        let state_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        let contract = search_index_contract(state_last_event_id, &metadata);
        let indexed_last_event_id = contract.indexed_last_event_id;
        let current_last_event_id = current_last_event_id(&conn, &board_id)?;
        if contract.mismatch || search_index_ahead(current_last_event_id, indexed_last_event_id) {
            drop(_write_guard);
            return rebuild_search_index(path_ref, board);
        }
        let lag = search_lag(current_last_event_id, indexed_last_event_id);
        if lag == 0 && state.as_ref().is_some_and(|state| !state.dirty) {
            return search_index_status(path_ref, board);
        }

        let now = SystemClock.now_ms();
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                indexed_last_event_id,
                true,
                Some("Tantivy task index sync is in progress".to_owned()),
                now,
            ),
        )?;
        mark_derived_store_dirty(&conn, TANTIVY_TASKS_STORE, now)?;

        let affected_task_ids =
            affected_task_ids_since(&conn, &board_id, indexed_last_event_id.unwrap_or(0))?;
        let documents = task_search_documents_for_task_ids(&conn, &board_id, &affected_task_ids)?;
        let metadata = match kanban_search::tantivy_backend::sync_task_index(
            &index_path,
            &board_id,
            current_last_event_id,
            &documents,
            &affected_task_ids,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                mark_derived_store_failure(
                    &conn,
                    TANTIVY_TASKS_STORE,
                    &board_id,
                    &error.to_string(),
                    SystemClock.now_ms(),
                )?;
                return Err(search_storage(error));
            }
        };
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                metadata.last_event_id,
                false,
                Some(format!(
                    "Synced Tantivy task index at {} ({} affected task(s))",
                    index_path.display(),
                    affected_task_ids.len()
                )),
                SystemClock.now_ms(),
            ),
        )?;
        mark_derived_store_success(
            &conn,
            TANTIVY_TASKS_STORE,
            &board_id,
            metadata.last_event_id,
            false,
            SystemClock.now_ms(),
        )?;
        Ok(SearchIndexStatus {
            backend: "tantivy".to_owned(),
            derived_index: true,
            stale: false,
            database_instance_id: None,
            protocol_version: None,
            generation: None,
            resolved_board_id: board_id,
            fallback_reason: None,
            index_version: Some(metadata.index_version),
            last_event_id: metadata.last_event_id,
            index_lag_events: Some(0),
            message: format!(
                "Synced Tantivy task index at {} ({} affected task(s))",
                index_path.display(),
                affected_task_ids.len()
            ),
        })
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        search_index_status(path, board)
    }
}

#[cfg(feature = "tantivy-backend")]
fn degraded_search_index_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    err: &kanban_search::tantivy_backend::TantivyTaskIndexError,
    resolved_board_id: &str,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        database_instance_id: None,
        protocol_version: None,
        generation: None,
        resolved_board_id: resolved_board_id.to_owned(),
        fallback_reason: Some("legacy_index_unavailable".to_owned()),
        index_version: None,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} is degraded ({:?}: {}); SQLite fallback search is active",
            index_path.display(),
            err.kind(),
            err
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
fn search_index_ahead_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    index_version: Option<String>,
    resolved_board_id: &str,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        database_instance_id: None,
        protocol_version: None,
        generation: None,
        resolved_board_id: resolved_board_id.to_owned(),
        fallback_reason: Some("legacy_index_ahead".to_owned()),
        index_version,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} is ahead of the database (indexed last_event_id {:?}, current last_event_id {:?}); SQLite fallback search is active until rebuild",
            index_path.display(),
            indexed_last_event_id,
            current_last_event_id
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
fn mismatched_search_index_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    index_version: Option<String>,
    state_last_event_id: Option<i64>,
    metadata_last_event_id: Option<i64>,
    resolved_board_id: &str,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        database_instance_id: None,
        protocol_version: None,
        generation: None,
        resolved_board_id: resolved_board_id.to_owned(),
        fallback_reason: Some("legacy_index_metadata_mismatch".to_owned()),
        index_version,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} has mismatched search state and metadata watermarks (state last_event_id {:?}, metadata last_event_id {:?}); SQLite fallback search is active until rebuild",
            index_path.display(),
            state_last_event_id,
            metadata_last_event_id
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
#[derive(Debug, Clone, Copy)]
struct SearchIndexContract {
    indexed_last_event_id: Option<i64>,
    mismatch: bool,
}

#[cfg(feature = "tantivy-backend")]
fn search_index_contract(
    state_last_event_id: Option<i64>,
    metadata: &kanban_search::tantivy_backend::TantivyIndexMetadata,
) -> SearchIndexContract {
    let metadata_last_event_id = metadata.last_event_id;
    let mismatch = matches!(
        (state_last_event_id, metadata_last_event_id),
        (Some(state), Some(metadata)) if state != metadata
    );
    SearchIndexContract {
        indexed_last_event_id: max_event_id(state_last_event_id, metadata_last_event_id),
        mismatch,
    }
}

#[cfg(feature = "tantivy-backend")]
fn max_event_id(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(feature = "tantivy-backend")]
fn tantivy_sqlite_fallback_required(query: &SearchQuery) -> bool {
    query.labels.iter().any(|label| !label.trim().is_empty())
        || query.q.as_deref().map(str::trim).is_some_and(|q| {
            task_ref_filter(q, "t.").is_some()
                || q.chars().any(|ch| {
                    matches!(
                        ch,
                        '"' | '+'
                            | '-'
                            | '!'
                            | '('
                            | ')'
                            | '{'
                            | '}'
                            | '['
                            | ']'
                            | '^'
                            | '~'
                            | '*'
                            | '?'
                            | ':'
                            | '\\'
                            | '/'
                            | '&'
                            | '|'
                            | '%'
                            | '_'
                    )
                })
        })
}

fn sqlite_search_meta(
    current_last_event_id: Option<i64>,
    stale: bool,
    indexed_last_event_id: Option<i64>,
    resolved_board_id: &str,
    projection: SearchProjectionContext,
) -> SearchMeta {
    SearchMeta {
        backend: "sqlite".to_owned(),
        stale,
        database_instance_id: projection.database_instance_id,
        protocol_version: projection.protocol_version,
        generation: projection.generation,
        resolved_board_id: resolved_board_id.to_owned(),
        fallback_reason: projection.fallback_reason,
        index_version: None,
        last_event_id: if stale {
            indexed_last_event_id
        } else {
            current_last_event_id
        },
        index_lag_events: Some(if stale {
            search_lag(current_last_event_id, indexed_last_event_id)
        } else {
            0
        }),
    }
}

fn search_task_where(board_id: &str, query: &SearchQuery) -> (String, Vec<Value>) {
    let mut clauses = vec!["WHERE t.board_id=?".to_owned()];
    let mut params = vec![Value::Text(board_id.to_owned())];
    if !query.include_archived {
        clauses.push("t.status != 'archived'".to_owned());
    }
    if !query.statuses.is_empty() {
        let placeholders = std::iter::repeat_n("?", query.statuses.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("t.status IN ({placeholders})"));
        params.extend(
            query
                .statuses
                .iter()
                .map(|status| Value::Text(status.as_str().to_owned())),
        );
    }
    for label in query
        .labels
        .iter()
        .map(|label| label.trim())
        .filter(|label| !label.is_empty())
    {
        clauses.push(
            "EXISTS (SELECT 1 FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.task_id=t.id AND tl.board_id=t.board_id AND l.board_id=t.board_id AND (l.name=? OR l.id=?))"
                .to_owned(),
        );
        params.push(Value::Text(label.to_owned()));
        params.push(Value::Text(label.to_owned()));
    }
    if let Some(assignee) = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("t.assignee=?".to_owned());
        params.push(Value::Text(assignee.to_owned()));
    }
    if let Some(search) = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some((clause, search_params)) = task_ref_filter(search, "t.")
    {
        clauses.push(clause);
        params.extend(search_params);
    }
    (clauses.join(" AND "), params)
}

#[cfg(feature = "tantivy-backend")]
fn search_state_key(board_id: &str) -> String {
    format!("{SEARCH_TASKS_STATE_KEY_PREFIX}.{board_id}")
}

#[cfg(feature = "tantivy-backend")]
fn default_search_index_state(
    board_id: &str,
    last_event_id: Option<i64>,
    dirty: bool,
    message: Option<String>,
    now: i64,
) -> SearchIndexState {
    SearchIndexState {
        schema_version: SEARCH_TASKS_STATE_SCHEMA_VERSION,
        index_version: kanban_search::tantivy_backend::INDEX_VERSION.to_owned(),
        backend: "tantivy".to_owned(),
        index_name: "tasks".to_owned(),
        board_id: board_id.to_owned(),
        last_event_id,
        dirty,
        updated_at: now,
        message,
    }
}

#[cfg(feature = "tantivy-backend")]
fn read_search_index_state(conn: &Connection, board_id: &str) -> Result<Option<SearchIndexState>> {
    let key = search_state_key(board_id);
    let value = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key=?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|err| KanbanError::Storage(err.to_string()))
        })
        .transpose()
}

#[cfg(feature = "tantivy-backend")]
fn write_search_index_state(conn: &Connection, state: &SearchIndexState) -> Result<()> {
    let key = search_state_key(&state.board_id);
    conn.execute(
        "INSERT INTO app_settings(key,value_json,updated_at) VALUES (?1,?2,?3) \
         ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at",
        params![
            key,
            serde_json::to_string(state).map_err(|err| KanbanError::Storage(err.to_string()))?,
            state.updated_at
        ],
    )
    .map_err(storage)?;
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
fn search_index_ahead(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
) -> bool {
    matches!(
        (current_last_event_id, indexed_last_event_id),
        (Some(current), Some(indexed)) if indexed > current
    )
}

#[cfg(feature = "tantivy-backend")]
fn task_index_path(db_path: &Path) -> PathBuf {
    kanban_local::task_index_path(db_path.to_path_buf())
}

#[cfg(feature = "tantivy-backend")]
fn task_search_documents(conn: &Connection, board_id: &str) -> Result<Vec<TaskSearchDocument>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id,t.board_id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,t.due_at,t.title,t.description,\
                    COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id ORDER BY c.created_at ASC, c.id ASC), '') AS comments,\
                    COALESCE((SELECT group_concat(COALESCE(r.summary, '') || ' ' || COALESCE(r.error, ''), char(10)) FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id ORDER BY r.started_at ASC, r.id ASC), '') AS run_text,\
                    COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10)) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id ORDER BY e.id ASC), '') AS event_text \
             FROM tasks t WHERE t.board_id=?1 ORDER BY t.seq ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], |row| {
            let status: String = row.get(3)?;
            Ok(TaskSearchDocument {
                task_id: row.get(0)?,
                board_id: row.get(1)?,
                seq: row.get(2)?,
                status: TaskStatus::try_from(status.as_str())
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                assignee: row.get(4)?,
                priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                due_at: row.get(8)?,
                title: row.get(9)?,
                description: row.get(10)?,
                comments: row.get(11)?,
                run_text: row.get(12)?,
                event_text: row.get(13)?,
            })
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "tantivy-backend")]
pub(super) fn task_search_documents_for_task_ids(
    conn: &Connection,
    board_id: &str,
    task_ids: &[String],
) -> Result<Vec<TaskSearchDocument>> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", task_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT t.id,t.board_id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,t.due_at,t.title,t.description,\
                COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id ORDER BY c.created_at ASC, c.id ASC), '') AS comments,\
                COALESCE((SELECT group_concat(COALESCE(r.summary, '') || ' ' || COALESCE(r.error, ''), char(10)) FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id ORDER BY r.started_at ASC, r.id ASC), '') AS run_text,\
                COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10)) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id ORDER BY e.id ASC), '') AS event_text \
         FROM tasks t WHERE t.board_id=? AND t.id IN ({placeholders}) ORDER BY t.seq ASC"
    );
    let mut params = vec![Value::Text(board_id.to_owned())];
    params.extend(task_ids.iter().cloned().map(Value::Text));
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            let status: String = row.get(3)?;
            Ok(TaskSearchDocument {
                task_id: row.get(0)?,
                board_id: row.get(1)?,
                seq: row.get(2)?,
                status: TaskStatus::try_from(status.as_str())
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                assignee: row.get(4)?,
                priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                due_at: row.get(8)?,
                title: row.get(9)?,
                description: row.get(10)?,
                comments: row.get(11)?,
                run_text: row.get(12)?,
                event_text: row.get(13)?,
            })
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "tantivy-backend")]
fn affected_task_ids_since(
    conn: &Connection,
    board_id: &str,
    last_event_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT task_id, run_id FROM task_events \
             WHERE board_id=?1 AND id>?2 ORDER BY id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, last_event_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .map_err(storage)?;
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let (task_id, run_id) = row.map_err(storage)?;
        let task_id = match task_id {
            Some(task_id) => Some(task_id),
            None => run_id
                .as_deref()
                .map(|run_id| {
                    conn.query_row(
                        "SELECT task_id FROM task_runs WHERE board_id=?1 AND id=?2",
                        params![board_id, run_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(storage)
                })
                .transpose()?
                .flatten(),
        };
        if let Some(task_id) = task_id
            && seen.insert(task_id.clone())
        {
            ids.push(task_id);
        }
    }
    Ok(ids)
}

#[cfg(feature = "tantivy-backend")]
fn search_storage(err: impl std::error::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}
