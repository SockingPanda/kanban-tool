use std::{collections::BTreeMap, env, sync::Arc};

use kanban_client::{DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::{
    ApiCreateTaskStatus, CreateTaskRequest, CreateTaskResponse, ListBoardsResponse,
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
