use super::list_support::*;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    routing::get,
};
use kanban_application::TaskListOptions as ApplicationTaskListOptions;
use kanban_contract::{ListTasksPath, ListTasksResponse, TotalPaginationMeta};

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(ListTasksPath { board }): Path<ListTasksPath>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<ListTasksResponse>, ApiError> {
    let query = parse_list_tasks_query(raw_query.as_deref())?;
    let options = ApplicationTaskListOptions {
        statuses: query.status.into_iter().map(task_status).collect(),
        priorities: query
            .priority
            .into_iter()
            .map(|priority| i64::from(priority.get()))
            .collect(),
        labels: query
            .label
            .into_iter()
            .map(|label| label.into_string())
            .collect(),
        plan_filters: query
            .plan_filter
            .into_iter()
            .map(application_plan_filter)
            .collect(),
        assignee: query.assignee,
        query: query.q,
        include_archived: query.include_archived,
        limit: query.limit,
        offset: query.offset,
        sort: application_task_sort(query.sort),
    };
    let page = state.application().list_tasks(&board, options).await?;
    let tasks = page
        .tasks
        .into_iter()
        .map(api_task)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListTasksResponse {
        data: tasks,
        meta: TotalPaginationMeta {
            limit: query.limit,
            offset: query.offset,
            total: page.total,
        },
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards/:board/tasks", get(list_tasks))
}
#[cfg(test)]
mod tests {}
