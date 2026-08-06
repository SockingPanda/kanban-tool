use kanban_protocol::{
    ApiTaskPriority, ApiTaskStatus, ListTasksQuery, ListTasksResponse, TaskReadLabel,
    TaskReadPlanFilter, TaskReadSort,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct TaskListArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
    status: Vec<ApiTaskStatus>,
    priority: Vec<i64>,
    label: Vec<String>,
    plan_filter: Vec<TaskReadPlanFilter>,
    assignee: Option<String>,
    query: Option<String>,
    include_archived: bool,
    #[serde(default = "default_list_limit")]
    limit: usize,
    offset: usize,
    sort: TaskReadSort,
}

const fn default_list_limit() -> usize {
    100
}

#[tool_router(router = task_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_list",
        description = "通过 canonical kanban application service 列出任务"
    )]
    async fn task_list(
        &self,
        Parameters(args): Parameters<TaskListArgs>,
    ) -> Result<Json<ListTasksResponse>, McpError> {
        let priority = args
            .priority
            .into_iter()
            .map(|value| {
                ApiTaskPriority::try_from(value).map_err(|value| {
                    McpError::invalid_params(
                        format!("priority 必须在 0 到 3 之间，当前值为 {value}"),
                        None,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let query = ListTasksQuery {
            status: args.status,
            priority,
            label: args
                .label
                .into_iter()
                .map(|value| {
                    TaskReadLabel::new(value.clone()).ok_or_else(|| {
                        McpError::invalid_params(format!("label 无效：{value}"), None)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            plan_filter: args.plan_filter,
            assignee: args.assignee,
            q: args.query,
            include_archived: args.include_archived,
            limit: args.limit,
            offset: args.offset,
            sort: args.sort,
        };
        let board = self.board(args.board);
        let client = self.client.clone();
        let response = call_client(move || client.list_tasks(&board, &query)).await?;
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_list");
    }
}
