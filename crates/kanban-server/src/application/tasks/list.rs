use super::list_support::*;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    ListTasksByStatusData, ListTasksByStatusPath, ListTasksByStatusQuery,
    ListTasksByStatusResponse, ListTasksPath, ListTasksQuery, ListTasksResponse,
    ListTasksStatusWindow, OffsetPaginationMeta, TotalPaginationMeta,
};
use kanban_service::TaskListOptions as ApplicationTaskListOptions;
pub(crate) async fn list_tasks(
    state: AppState,
    ListTasksPath { board }: ListTasksPath,
    mut query: ListTasksQuery,
) -> Result<ListTasksResponse, ApiError> {
    validate_list_query(&mut query)?;
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
    Ok(ListTasksResponse {
        data: tasks,
        meta: TotalPaginationMeta {
            limit: query.limit,
            offset: query.offset,
            total: page.total,
        },
    })
}
pub(crate) async fn list_tasks_by_status(
    state: AppState,
    ListTasksByStatusPath { board }: ListTasksByStatusPath,
    mut query: ListTasksByStatusQuery,
) -> Result<ListTasksByStatusResponse, ApiError> {
    validate_status_query(&mut query)?;
    let priorities = query
        .priority
        .iter()
        .map(|priority| i64::from(priority.get()))
        .collect::<Vec<_>>();
    let labels = query
        .label
        .iter()
        .map(|label| label.as_str().to_owned())
        .collect::<Vec<_>>();
    let plan_filters = query
        .plan_filter
        .iter()
        .copied()
        .map(application_plan_filter)
        .collect::<Vec<_>>();
    let sort = application_task_sort(query.sort);
    let mut statuses = Vec::with_capacity(query.status.len());
    for status in query.status {
        let page = state
            .application()
            .list_tasks(
                &board,
                ApplicationTaskListOptions {
                    statuses: vec![task_status(status)],
                    priorities: priorities.clone(),
                    labels: labels.clone(),
                    plan_filters: plan_filters.clone(),
                    assignee: query.assignee.clone(),
                    query: query.q.clone(),
                    include_archived: query.include_archived,
                    limit: query.limit,
                    offset: query.offset,
                    sort,
                },
            )
            .await?;
        let tasks = page
            .tasks
            .into_iter()
            .map(api_task)
            .collect::<Result<Vec<_>, _>>()?;
        statuses.push(ListTasksStatusWindow {
            status,
            tasks,
            page: TotalPaginationMeta {
                limit: query.limit,
                offset: query.offset,
                total: page.total,
            },
        });
    }
    Ok(ListTasksByStatusResponse {
        data: ListTasksByStatusData { statuses },
        meta: OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    })
}
