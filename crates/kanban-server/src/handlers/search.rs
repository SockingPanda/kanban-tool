use axum::{
    Json,
    extract::{Query, RawQuery, State, rejection::QueryRejection},
};
use kanban_core::TaskStatus;
use kanban_search::SearchQuery;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::{Envelope, SearchTaskHitDto, SearchTasksDto, TaskDto};
use crate::error::{ApiError, extractor_error, validate_page_bounds};
use crate::state::AppState;

use super::{
    shared::{default_board, parse_status_filters},
    tasks::parse_label_filters,
};

#[derive(Debug, Deserialize)]
pub(crate) struct SearchTasksQuery {
    #[serde(default = "default_board")]
    board: String,
    q: Option<String>,
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_search_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchStatusQuery {
    #[serde(default = "default_board")]
    board: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchPageMetaDto {
    limit: usize,
    offset: usize,
    total: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchTaskStatusWindowDto {
    status: TaskStatus,
    tasks: Vec<TaskDto>,
    search_meta: kanban_search::SearchMeta,
    page: SearchPageMetaDto,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchTaskStatusWindowsDto {
    statuses: Vec<SearchTaskStatusWindowDto>,
}

fn default_search_limit() -> usize {
    20
}

pub(crate) async fn search_tasks(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<SearchTasksQuery>, QueryRejection>,
) -> Result<Json<Envelope<SearchTasksDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_SEARCH_LIMIT, query.offset)?;
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let labels = parse_label_filters(raw_query.as_deref())?;
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
    let results = kanban_sqlite::search_tasks(
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
            let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &hit.task_id)?;
            Ok(SearchTaskHitDto {
                task_id: hit.task_id,
                seq: hit.seq,
                score: hit.score,
                snippet: hit.snippet,
                task: TaskDto::from(task),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(Envelope {
        data: SearchTasksDto {
            hits,
            meta: results.meta,
        },
        meta: Some(json!({ "limit": query.limit, "offset": query.offset })),
    }))
}

pub(crate) async fn search_tasks_by_status(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<SearchTasksQuery>, QueryRejection>,
) -> Result<Json<Envelope<SearchTaskStatusWindowsDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_SEARCH_LIMIT, query.offset)?;
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let labels = parse_label_filters(raw_query.as_deref())?;
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
        let results = kanban_sqlite::search_tasks(
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
            .map(|hit| kanban_sqlite::get_task_by_id_global(state.db_path(), &hit.task_id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(TaskDto::from)
            .collect();
        windows.push(SearchTaskStatusWindowDto {
            status,
            tasks,
            search_meta: results.meta,
            page: SearchPageMetaDto {
                limit: query.limit,
                offset: query.offset,
                total: None,
            },
        });
    }
    Ok(Json(Envelope {
        data: SearchTaskStatusWindowsDto { statuses: windows },
        meta: Some(json!({ "limit": query.limit, "offset": query.offset })),
    }))
}

pub(crate) async fn search_status(
    State(state): State<AppState>,
    query: Result<Query<SearchStatusQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_search::SearchIndexStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::search_index_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}
