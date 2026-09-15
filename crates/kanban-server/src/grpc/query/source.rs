//! 明确的 typed 查询接线。所有投影读取复用现有 application query 与业务 response codec。
use super::super::context::{invalid_request, response_codec_error, service_error};
use crate::{AppState, application as app};
use kanban_protocol::rpc::v1::{
    self as pb, query_definition::Query, query_result::Result as Projection,
};
use prost::Message;
use sha2::{Digest, Sha256};
use tonic::Status;

macro_rules! query_calls {
    ($( $variant:ident($request:ident) ($state:ident; $path:pat, $query:pat) => $call:expr; )*) => {
        fn defaults(query: Query) -> Result<Query, Status> {
            match query {
                $(Query::$variant(request) => {
                    let (path, query, input) = request.decode_parts().map_err(invalid_request)?;
                    Ok(Query::$variant(pb::$request::from_parts(path, query, input).map_err(invalid_request)?))
                })*
                Query::RecentEvents(request) => Ok(Query::RecentEvents(request)),
            }
        }

        async fn result(state: AppState, query: Query) -> Result<pb::QueryResult, Status> {
            let result = match query {
                $(Query::$variant(request) => {
                    let ($path, $query, _) = request.decode_parts().map_err(invalid_request)?;
                    let $state = state;
                    Projection::$variant($call.await.map_err(service_error)?.try_into().map_err(response_codec_error)?)
                })*
                Query::RecentEvents(request) => {
                    let page = state.application().recent_events(&request.board_id, request.task_id.as_deref(), request.limit as usize)
                        .await.map_err(|error| service_error(error.into()))?;
                    let events = page.events.into_iter().map(app::events::list::api_event)
                        .collect::<Result<Vec<_>, _>>().map_err(service_error)?;
                    let value = kanban_protocol::ListEventsResponse::new(events, kanban_protocol::NextAfterMeta { next_after: page.next_after });
                    Projection::RecentEvents(value.try_into().map_err(response_codec_error)?)
                }
            };
            let result = pb::QueryResult { result: Some(result) };
            if result.encoded_len() > super::MAX_SNAPSHOT_BYTES {
                return Err(Status::resource_exhausted("完整查询快照超过 64 MiB"));
            }
            Ok(result)
        }
    };
}

pub(super) struct Sample {
    pub bytes: Vec<u8>,
    pub semantic_hash: [u8; 32],
}

pub(super) async fn sample(state: AppState, query: Query) -> Result<Sample, Status> {
    let mut result = result(state, query).await?;
    // 先保存完整真实响应。以下临时比较键只忽略样本生成时间，从不进入 wire 或缓存。
    let bytes = result.encode_to_vec();
    let timestamp = match result.result.as_mut() {
        Some(Projection::GetStats(value)) => value.data.as_mut().map(|data| &mut data.generated_at),
        Some(Projection::TaskNeighborhood(value)) => value
            .data
            .as_mut()
            .and_then(|data| data.meta.as_mut())
            .map(|meta| &mut meta.generated_at),
        Some(Projection::BoardTaskMap(value)) => value
            .data
            .as_mut()
            .and_then(|data| data.meta.as_mut())
            .map(|meta| &mut meta.generated_at),
        _ => None,
    };
    let semantic_hash = if let Some(timestamp) = timestamp {
        *timestamp = Some(0);
        Sha256::digest(result.encode_to_vec()).into()
    } else {
        Sha256::digest(&bytes).into()
    };
    Ok(Sample {
        bytes,
        semantic_hash,
    })
}

#[cfg(test)]
pub(super) async fn read(state: AppState, query: Query) -> Result<Vec<u8>, Status> {
    Ok(result(state, query).await?.encode_to_vec())
}

query_calls! {
    GetHealth(GetHealthRequest)(state; _, _) => app::health::get_health(state);
    ListBoards(ListBoardsRequest)(state; _, query) => app::boards::list::list_boards(state, query);
    GetBoard(GetBoardRequest)(state; path, _) => app::boards::get::get_board(state, path);
    ListBoardColumns(ListBoardColumnsRequest)(state; path, _) => app::boards::columns::list_board_columns(state, path);
    ListTasks(ListTasksRequest)(state; path, query) => app::tasks::list::list_tasks(state, path, query);
    ListTasksByStatus(ListTasksByStatusRequest)(state; path, query) => app::tasks::list::list_tasks_by_status(state, path, query);
    GetTask(GetTaskRequest)(state; path, query) => app::tasks::show::get_task(state, path, query);
    GetTaskDetails(GetTaskDetailsRequest)(state; path, _) => app::tasks::show::get_task_details(state, path);
    ListTaskLabels(ListTaskLabelsRequest)(state; path, _) => app::labels::list_task_labels(state, path);
    ListBoardLabels(ListBoardLabelsRequest)(state; path, _) => app::labels::list_board_labels(state, path);
    ListDependencies(ListDependenciesRequest)(state; path, _) => app::dependencies::list::list_dependencies(state, path);
    ListSteps(ListStepsRequest)(state; path, _) => app::steps::list::list_steps(state, path);
    ListComments(ListCommentsRequest)(state; path, _) => app::comments::list::list_comments(state, path);
    ListAttachments(ListAttachmentsRequest)(state; path, _) => app::attachments::list_attachments(state, path);
    TaskNeighborhood(TaskNeighborhoodRequest)(state; path, query) => app::graph::task_neighborhood(state, path, query);
    BoardTaskMap(BoardTaskMapRequest)(state; path, query) => app::graph::board_task_map(state, path, query);
    ListRuns(ListRunsRequest)(state; path, _) => app::runs::list::list_runs(state, path);
    GetRun(GetRunRequest)(state; path, _) => app::runs::show::get_run(state, path);
    GetRunLog(GetRunLogRequest)(state; path, _) => app::runs::log::get_run_log(state, path);
    ListEvents(ListEventsRequest)(state; _, query) => app::events::list::list_events(state, query);
    GetStats(GetStatsRequest)(state; _, query) => app::stats::stats(state, query);
    MaintenanceStatus(MaintenanceStatusRequest)(state; _, _) => app::maintenance::maintenance_status(state);
    SearchStatus(SearchStatusRequest)(state; _, query) => app::search::search_status(state, query);
    SearchTasks(SearchTasksRequest)(state; _, query) => app::search::search_tasks(state, query);
    SearchTasksByStatus(SearchTasksByStatusRequest)(state; _, query) => app::search::search_tasks_by_status(state, query);
}

pub(super) async fn normalize(state: &AppState, query: Query) -> Result<Query, Status> {
    let mut query = defaults(query)?;
    macro_rules! board {
        ($($variant:ident),*) => {
            match &mut query {
                $(Query::$variant(request) => canonical_board(state, &mut request.board).await?,)*
                _ => {}
            }
        };
    }
    board!(
        GetBoard,
        ListBoardColumns,
        ListTasks,
        ListTasksByStatus,
        ListBoardLabels,
        BoardTaskMap,
        ListEvents,
        GetStats,
        SearchStatus,
        SearchTasks,
        SearchTasksByStatus
    );
    macro_rules! task {
        ($($variant:ident),*) => {
            match &mut query {
                $(Query::$variant(request) => trim_id(&mut request.task_id, "t_")?,)*
                _ => {}
            }
        };
    }
    task!(
        GetTask,
        GetTaskDetails,
        ListTaskLabels,
        ListDependencies,
        ListSteps,
        ListComments,
        ListAttachments,
        TaskNeighborhood,
        ListRuns
    );
    match &mut query {
        Query::GetRun(request) => trim_id(&mut request.run_id, "r_")?,
        Query::GetRunLog(request) => trim_id(&mut request.run_id, "r_")?,
        Query::GetTask(request) => {
            trim_optional(&mut request.include);
            if request.include.is_some() {
                return Err(invalid_request("详情请使用 GetTaskDetails query"));
            }
        }
        Query::ListTasks(request) => {
            let (_, mut options, _) = request.clone().decode_parts().map_err(invalid_request)?;
            app::tasks::list_support::validate_list_query(&mut options).map_err(service_error)?;
            trim_optional(&mut request.q);
            trim_optional(&mut request.assignee);
            request.status.sort_unstable();
            request.priority.sort_by_key(|value| value.value);
            request
                .label
                .sort_by(|left, right| left.value.cmp(&right.value));
            request.plan_filter.sort_unstable();
        }
        Query::ListTasksByStatus(request) => {
            let (_, mut options, _) = request.clone().decode_parts().map_err(invalid_request)?;
            app::tasks::list_support::validate_status_query(&mut options).map_err(service_error)?;
            trim_optional(&mut request.q);
            trim_optional(&mut request.assignee);
            // status 的顺序决定各窗口在 response 中的顺序，必须保留。
            request.priority.sort_by_key(|value| value.value);
            request
                .label
                .sort_by(|left, right| left.value.cmp(&right.value));
            request.plan_filter.sort_unstable();
        }
        Query::ListEvents(request) => {
            if request.after.is_some_and(|after| after < 0) {
                return Err(invalid_request("after 必须非负"));
            }
            if request.task_id.is_some() {
                trim_id(&mut request.task_id, "t_")?;
            }
            request.limit = request.limit.map(|limit| limit.min(1000));
        }
        Query::RecentEvents(request) => {
            if request.board_id.trim().is_empty() || request.limit > 1000 {
                return Err(invalid_request(
                    "RecentEvents 需要 board_id 且 limit <= 1000",
                ));
            }
            let mut board = Some(request.board_id.clone());
            canonical_board(state, &mut board).await?;
            request.board_id = board.unwrap();
            if request.task_id.is_some() {
                trim_id(&mut request.task_id, "t_")?;
            }
        }
        Query::SearchTasks(request) => {
            let (_, options, _) = request.clone().decode_parts().map_err(invalid_request)?;
            let normalized = normalized_search(&options)?;
            request.status.sort_unstable();
            request.label = normalized.labels;
            request.label.sort();
            request.q = normalized.q;
            request.assignee = normalized.assignee;
        }
        Query::SearchTasksByStatus(request) => {
            let (_, options, _) = request.clone().decode_parts().map_err(invalid_request)?;
            let normalized = normalized_search(&options)?;
            request.label = normalized.labels;
            request.label.sort();
            request.q = normalized.q;
            request.assignee = normalized.assignee;
        }
        _ => {}
    }
    Ok(query)
}

fn normalized_search(
    options: &kanban_protocol::SearchTasksQuery,
) -> Result<kanban_service::SearchQuery, Status> {
    let query = app::search::to_search_query(options, Vec::new());
    query.validate().map_err(invalid_request)?;
    Ok(query.normalized())
}

async fn canonical_board(state: &AppState, board: &mut Option<String>) -> Result<(), Status> {
    if let Some(selector) = board {
        *selector = selector.trim().to_owned();
        if selector.is_empty() {
            return Err(invalid_request("board 必须非空"));
        }
        if let Ok(resolved) = state.application().get_board(selector).await {
            *selector = resolved.id;
        }
    }
    Ok(())
}

fn trim_optional(value: &mut Option<String>) {
    *value = value
        .take()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
}

fn trim_id(value: &mut Option<String>, prefix: &str) -> Result<(), Status> {
    trim_optional(value);
    if !value
        .as_ref()
        .is_some_and(|value| value.starts_with(prefix) && value.len() > prefix.len())
    {
        return Err(invalid_request(format!("需要 canonical {prefix}... ID")));
    }
    Ok(())
}
