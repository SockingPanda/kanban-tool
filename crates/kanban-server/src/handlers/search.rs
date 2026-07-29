use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use kanban_contract::{
    BoardQuery, DataEnvelope, MetadataEnvelope, OffsetPaginationMeta, SearchMeta, SearchPageMeta,
    SearchStatusResponse, SearchTaskHit, SearchTaskStatusWindow, SearchTaskStatusWindows,
    SearchTasksByStatusResponse, SearchTasksData, SearchTasksQuery, SearchTasksResponse,
};
use kanban_search::SearchQuery;

use crate::dto::{api_task_from_record, api_task_status_from_core, task_status_from_api};
use crate::error::{ApiError, extractor_error, validate_page_bounds};
use crate::state::AppState;

pub(crate) async fn search_tasks(
    State(state): State<AppState>,
    query: Result<Query<SearchTasksQuery>, QueryRejection>,
) -> Result<Json<SearchTasksResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(
        query.limit,
        kanban_sqlite::api::MAX_SEARCH_LIMIT,
        query.offset,
    )?;
    let statuses = query
        .status
        .iter()
        .copied()
        .map(task_status_from_api)
        .collect();
    let labels: Vec<String> = query
        .label
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let results = kanban_sqlite::api::search_tasks(
        state.db_path(),
        SearchQuery {
            board: query.board,
            q,
            statuses,
            labels,
            assignee,
            include_archived: query.include_archived,
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let hits = results
        .hits
        .into_iter()
        .map(|hit| {
            let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &hit.task_id)?;
            Ok(SearchTaskHit {
                task_id: hit.task_id,
                seq: hit.seq,
                score: hit.score,
                snippet: hit.snippet,
                task: api_task_from_record(task)?,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(MetadataEnvelope::new(
        SearchTasksData {
            hits,
            meta: search_meta(results.meta),
        },
        OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    )))
}

pub(crate) async fn search_tasks_by_status(
    State(state): State<AppState>,
    query: Result<Query<SearchTasksQuery>, QueryRejection>,
) -> Result<Json<SearchTasksByStatusResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(
        query.limit,
        kanban_sqlite::api::MAX_SEARCH_LIMIT,
        query.offset,
    )?;
    let statuses = query
        .status
        .iter()
        .copied()
        .map(task_status_from_api)
        .collect::<Vec<_>>();
    let labels: Vec<String> = query
        .label
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut windows = Vec::with_capacity(statuses.len());
    for status in statuses {
        let results = kanban_sqlite::api::search_tasks(
            state.db_path(),
            SearchQuery {
                board: query.board.clone(),
                q: q.clone(),
                statuses: vec![status],
                labels: labels.clone(),
                assignee: assignee.clone(),
                include_archived: query.include_archived,
                limit: query.limit,
                offset: query.offset,
            },
        )?;
        let tasks = results
            .hits
            .into_iter()
            .map(|hit| kanban_sqlite::api::get_task_by_id_global(state.db_path(), &hit.task_id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(api_task_from_record)
            .collect::<Result<Vec<_>, _>>()?;
        windows.push(SearchTaskStatusWindow {
            status: api_task_status_from_core(status),
            tasks,
            search_meta: search_meta(results.meta),
            page: SearchPageMeta {
                limit: query.limit,
                offset: query.offset,
                total: None,
            },
        });
    }
    Ok(Json(MetadataEnvelope::new(
        SearchTaskStatusWindows { statuses: windows },
        OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    )))
}

pub(crate) async fn search_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<SearchStatusResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let status = kanban_sqlite::api::search_index_status(state.db_path(), &query.board)?;
    Ok(Json(DataEnvelope::new(crate::search_status_from_record(
        status,
    ))))
}

fn search_meta(value: kanban_search::SearchMeta) -> SearchMeta {
    SearchMeta {
        backend: value.backend,
        stale: value.stale,
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        generation: value.generation,
        resolved_board_id: value.resolved_board_id,
        fallback_reason: value.fallback_reason,
        index_version: value.index_version,
        last_event_id: value.last_event_id,
        index_lag_events: value.index_lag_events,
    }
}
