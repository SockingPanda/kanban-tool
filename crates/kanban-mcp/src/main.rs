use std::{collections::BTreeMap, env, sync::Arc};

use kanban_client::{DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::{
    ApiCreateTaskStatus, ApiTaskPriority, ApiTaskStatus, CreateTaskRequest, CreateTaskResponse,
    GetTaskResponse, ListBoardsResponse, ListTasksQuery, ListTasksResponse,
    MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse, TaskReadPlanFilter,
    TaskReadSort,
};
use rmcp::{
    ErrorData as McpError, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
    transport::stdio,
};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardListArgs {
    /// Include archived boards in the result.
    include_archived: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskCreateArgs {
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
    title: String,
    description: Option<String>,
    status: Option<ApiCreateTaskStatus>,
    assignee: Option<String>,
    #[serde(default = "default_priority")]
    priority: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Option<BTreeMap<String, serde_json::Value>>,
    task_id: Option<String>,
    idempotency_key: Option<String>,
}

const fn default_priority() -> i64 {
    3
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct TaskListArgs {
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
    status: Vec<ApiTaskStatus>,
    priority: Vec<i64>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskShowArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskPlanNotRequiredArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    reason: String,
}

#[derive(Clone)]
struct KanbanMcp {
    client: Arc<KanbanClient>,
    default_board: Arc<str>,
}

#[tool_router(server_handler)]
impl KanbanMcp {
    #[tool(
        name = "board_list",
        description = "List boards from the canonical kanban host"
    )]
    async fn board_list(
        &self,
        Parameters(args): Parameters<BoardListArgs>,
    ) -> Result<Json<ListBoardsResponse>, McpError> {
        let client = self.client.clone();
        let boards = tokio::task::spawn_blocking(move || client.list_boards(args.include_archived))
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;

        Ok(Json(ListBoardsResponse { data: boards }))
    }

    #[tool(
        name = "task_create",
        description = "Create a task through the canonical kanban application service"
    )]
    async fn task_create(
        &self,
        Parameters(args): Parameters<TaskCreateArgs>,
    ) -> Result<Json<CreateTaskResponse>, McpError> {
        let client = self.client.clone();
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let task = tokio::task::spawn_blocking(move || {
            client.create_task(
                &board,
                CreateTaskRequest {
                    task_id: args.task_id,
                    idempotency_key: args.idempotency_key,
                    title: args.title,
                    description: args.description,
                    status: args.status,
                    assignee: args.assignee,
                    priority: args.priority,
                    scheduled_at: args.scheduled_at,
                    due_at: args.due_at,
                    max_retries: args.max_retries,
                    metadata: args.metadata,
                    labels: Vec::new(),
                    depends_on: Vec::new(),
                    actor: None,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;

        Ok(Json(CreateTaskResponse { data: task }))
    }

    #[tool(
        name = "task_list",
        description = "List tasks through the canonical kanban application service"
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
                        format!("priority must be between 0 and 3, got {value}"),
                        None,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let query = ListTasksQuery {
            status: args.status,
            priority,
            label: Vec::new(),
            plan_filter: args.plan_filter,
            assignee: args.assignee,
            q: args.query,
            include_archived: args.include_archived,
            limit: args.limit,
            offset: args.offset,
            sort: args.sort,
        };
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let response = tokio::task::spawn_blocking(move || client.list_tasks(&board, &query))
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(response))
    }

    #[tool(
        name = "task_show",
        description = "Show one task through the canonical kanban application service"
    )]
    async fn task_show(
        &self,
        Parameters(args): Parameters<TaskShowArgs>,
    ) -> Result<Json<GetTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.get_task_by_selector(&board, &args.task_ref)
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(GetTaskResponse::new(task, None)))
    }

    #[tool(
        name = "task_plan_not_required",
        description = "Mark a task execution plan as not required through the canonical application service"
    )]
    async fn task_plan_not_required(
        &self,
        Parameters(args): Parameters<TaskPlanNotRequiredArgs>,
    ) -> Result<Json<MarkExecutionPlanNotRequiredResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let plan = tokio::task::spawn_blocking(move || {
            client.mark_execution_plan_not_required_by_selector(
                &board,
                &args.task_ref,
                &MarkExecutionPlanNotRequiredRequest {
                    reason: args.reason,
                    actor: None,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(MarkExecutionPlanNotRequiredResponse { data: plan }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_url =
        env::var("KANBAN_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
    let actor = env::var("KANBAN_ACTOR").unwrap_or_else(|_| "mcp".to_owned());
    let default_board = env::var("KB_BOARD").unwrap_or_else(|_| "default".to_owned());
    let service = KanbanMcp {
        client: Arc::new(KanbanClient::new(server_url, actor)?),
        default_board: Arc::from(default_board),
    }
    .serve(stdio())
    .await?;
    service.waiting().await?;
    Ok(())
}
