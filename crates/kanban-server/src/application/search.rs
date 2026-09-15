use crate::{application::tasks::support::api_task, error::ApiError, state::AppState};
use kanban_protocol::{
    ApiTaskStatus, BoardQuery, DataEnvelope, MetadataEnvelope, OffsetPaginationMeta, SearchMeta,
    SearchPageMeta, SearchStatus, SearchStatusResponse, SearchTaskHit, SearchTaskStatusWindow,
    SearchTaskStatusWindows, SearchTasksByStatusResponse, SearchTasksData, SearchTasksQuery,
    SearchTasksResponse,
};
use kanban_service::TaskStatus;
use kanban_service::{SearchIndexStatus, SearchMeta as AppSearchMeta, SearchQuery};
pub(crate) async fn search_tasks(
    state: AppState,
    query: SearchTasksQuery,
) -> Result<SearchTasksResponse, ApiError> {
    let results = state
        .application()
        .search_tasks(to_search_query(&query, Vec::new()))
        .await?;
    let mut hits = Vec::with_capacity(results.hits.len());
    for hit in results.hits {
        let task = state.application().get_task(&hit.task_id).await?;
        hits.push(SearchTaskHit {
            task_id: hit.task_id,
            seq: hit.seq,
            score: hit.score,
            snippet: hit.snippet,
            task: api_task(task)?,
        });
    }
    Ok(MetadataEnvelope::new(
        SearchTasksData {
            hits,
            meta: search_meta(results.meta),
        },
        OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    ))
}
pub(crate) async fn search_tasks_by_status(
    state: AppState,
    query: SearchTasksQuery,
) -> Result<SearchTasksByStatusResponse, ApiError> {
    let mut windows = Vec::with_capacity(query.status.len());
    for status in query.status.iter().copied() {
        let results = state
            .application()
            .search_tasks(to_search_query(&query, vec![task_status(status)]))
            .await?;
        let mut tasks = Vec::with_capacity(results.hits.len());
        for hit in results.hits {
            tasks.push(api_task(state.application().get_task(&hit.task_id).await?)?);
        }
        windows.push(SearchTaskStatusWindow {
            status,
            tasks,
            search_meta: search_meta(results.meta),
            page: SearchPageMeta {
                limit: query.limit,
                offset: query.offset,
                total: None,
            },
        });
    }
    Ok(MetadataEnvelope::new(
        SearchTaskStatusWindows { statuses: windows },
        OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    ))
}
pub(crate) async fn search_status(
    state: AppState,
    query: BoardQuery,
) -> Result<SearchStatusResponse, ApiError> {
    let status = state
        .application()
        .search_index_status(&query.board)
        .await?;
    Ok(DataEnvelope::new(to_search_status(status)))
}
pub(crate) async fn rebuild_search(
    state: AppState,
    query: BoardQuery,
) -> Result<SearchStatusResponse, ApiError> {
    Ok(DataEnvelope::new(to_search_status(
        state
            .application()
            .rebuild_search_index(&query.board)
            .await?,
    )))
}
pub(crate) async fn sync_search(
    state: AppState,
    query: BoardQuery,
) -> Result<SearchStatusResponse, ApiError> {
    Ok(DataEnvelope::new(to_search_status(
        state.application().sync_search_index(&query.board).await?,
    )))
}
fn to_search_query(query: &SearchTasksQuery, statuses: Vec<TaskStatus>) -> SearchQuery {
    SearchQuery {
        board: query.board.clone(),
        q: query.q.clone(),
        statuses: if statuses.is_empty() {
            query.status.iter().copied().map(task_status).collect()
        } else {
            statuses
        },
        labels: query.label.clone(),
        assignee: query.assignee.clone(),
        include_archived: query.include_archived,
        limit: query.limit,
        offset: query.offset,
    }
}
fn search_meta(value: AppSearchMeta) -> SearchMeta {
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
fn to_search_status(value: SearchIndexStatus) -> SearchStatus {
    SearchStatus {
        backend: value.backend,
        derived_index: value.derived_index,
        stale: value.stale,
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        generation: value.generation,
        resolved_board_id: value.resolved_board_id,
        fallback_reason: value.fallback_reason,
        index_version: value.index_version,
        last_event_id: value.last_event_id,
        index_lag_events: value.index_lag_events,
        message: value.message,
    }
}
fn task_status(status: ApiTaskStatus) -> TaskStatus {
    match status {
        ApiTaskStatus::Triage => TaskStatus::Triage,
        ApiTaskStatus::Todo => TaskStatus::Todo,
        ApiTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiTaskStatus::Ready => TaskStatus::Ready,
        ApiTaskStatus::Running => TaskStatus::Running,
        ApiTaskStatus::Blocked => TaskStatus::Blocked,
        ApiTaskStatus::Review => TaskStatus::Review,
        ApiTaskStatus::Done => TaskStatus::Done,
        ApiTaskStatus::Archived => TaskStatus::Archived,
    }
}
