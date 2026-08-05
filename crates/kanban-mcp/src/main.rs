use std::{collections::BTreeMap, env, sync::Arc};

use kanban_client::{DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::{
    ApiCreateTaskStatus, ApiTaskPriority, ApiTaskStatus, BlockTaskRequest, BlockTaskResponse,
    ClaimTaskRequest, ClaimTaskResponse, CommentAuthorType, CommentKind, CompleteTaskRequest,
    CompleteTaskResponse, CreateCommentRequest, CreateCommentResponse, CreateTaskRequest,
    CreateTaskResponse, GetTaskResponse, HeartbeatTaskRequest, HeartbeatTaskResponse,
    ListBoardsResponse, ListTasksQuery, ListTasksResponse, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse, PromoteTaskRequest, PromoteTaskResponse,
    ReleaseTaskRequest, ReleaseTaskResponse, SubmitReviewTaskRequest, SubmitReviewTaskResponse,
    TaskReadPlanFilter, TaskReadSort,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskPromoteArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskClaimArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Claim lease duration in milliseconds.
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// Worker configuration recorded on the run. Defaults to manual.
    worker_profile: Option<String>,
    /// JSON metadata recorded on the run and claimed event.
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskHeartbeatArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Exact token returned by task_claim.
    claim_token: String,
    /// New claim lease duration in milliseconds.
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// Optional heartbeat note recorded on the event.
    note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskReleaseArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Exact token returned by task_claim.
    claim_token: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskReviewArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Exact token returned by task_claim. May be omitted only with force.
    claim_token: Option<String>,
    /// Bypass caller credential checks without bypassing running-run consistency.
    #[serde(default)]
    force: bool,
    /// Optional summary recorded on the task and completed run.
    summary: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskDoneArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Exact token returned by task_claim when completing from running.
    claim_token: Option<String>,
    /// Bypass running caller credentials without bypassing required-step guards.
    #[serde(default)]
    force: bool,
    /// Optional summary stored on the task and active run.
    summary: Option<String>,
    /// Optional opaque JSON result stored on the task and completion event.
    result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskBlockArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Required reason recorded on the task, failed run, and event.
    reason: String,
    /// Exact token returned by task_claim when blocking from running.
    claim_token: Option<String>,
    /// Bypass running caller credentials without bypassing task/run consistency.
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommentCreateArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    body: String,
    kind: Option<CommentKind>,
    author: Option<String>,
    author_type: Option<CommentAuthorType>,
    agent_type: Option<String>,
    metadata: Option<BTreeMap<String, serde_json::Value>>,
    idempotency_key: Option<String>,
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
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

    #[tool(
        name = "task_promote",
        description = "Promote an eligible task to ready through the canonical application service"
    )]
    async fn task_promote(
        &self,
        Parameters(args): Parameters<TaskPromoteArgs>,
    ) -> Result<Json<PromoteTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.promote_task_by_selector(
                &board,
                &args.task_ref,
                &PromoteTaskRequest { actor: None },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(PromoteTaskResponse::new(task)))
    }

    #[tool(
        name = "comment_create",
        description = "Create a note or decision comment through the canonical application service"
    )]
    async fn comment_create(
        &self,
        Parameters(args): Parameters<CommentCreateArgs>,
    ) -> Result<Json<CreateCommentResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let request = CreateCommentRequest {
            idempotency_key: args.idempotency_key,
            author: args.author,
            body: args.body,
            kind: args.kind,
            author_type: args.author_type,
            agent_type: args.agent_type,
            metadata: args
                .metadata
                .map(|metadata| serde_json::Value::Object(metadata.into_iter().collect())),
        };
        let comment = tokio::task::spawn_blocking(move || {
            client.create_comment_by_selector(&board, &task_ref, &request)
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(CreateCommentResponse { data: comment }))
    }

    #[tool(
        name = "task_claim",
        description = "Atomically claim a ready task and create its run through the canonical application service"
    )]
    async fn task_claim(
        &self,
        Parameters(args): Parameters<TaskClaimArgs>,
    ) -> Result<Json<ClaimTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let claim = tokio::task::spawn_blocking(move || {
            client.claim_task_by_selector(
                &board,
                &args.task_ref,
                &ClaimTaskRequest {
                    actor: None,
                    ttl_ms: args.ttl_ms,
                    worker_profile: args.worker_profile,
                    metadata: args.metadata,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(ClaimTaskResponse::new(claim)))
    }

    #[tool(
        name = "task_heartbeat",
        description = "Extend an active claim lease through the canonical application service"
    )]
    async fn task_heartbeat(
        &self,
        Parameters(args): Parameters<TaskHeartbeatArgs>,
    ) -> Result<Json<HeartbeatTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.heartbeat_task_by_selector(
                &board,
                &args.task_ref,
                &HeartbeatTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                    ttl_ms: args.ttl_ms,
                    note: args.note,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(HeartbeatTaskResponse::new(task)))
    }

    #[tool(
        name = "task_release",
        description = "Return an actively claimed task to ready through the canonical application service"
    )]
    async fn task_release(
        &self,
        Parameters(args): Parameters<TaskReleaseArgs>,
    ) -> Result<Json<ReleaseTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.release_task_by_selector(
                &board,
                &args.task_ref,
                &ReleaseTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(ReleaseTaskResponse::new(task)))
    }

    #[tool(
        name = "task_review",
        description = "Finish an active run and submit its task for review through the canonical application service"
    )]
    async fn task_review(
        &self,
        Parameters(args): Parameters<TaskReviewArgs>,
    ) -> Result<Json<SubmitReviewTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.submit_review_task_by_selector(
                &board,
                &args.task_ref,
                &SubmitReviewTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                    force: args.force,
                    summary: args.summary,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(SubmitReviewTaskResponse::new(task)))
    }

    #[tool(
        name = "task_done",
        description = "Complete a running or reviewed task through the canonical application service"
    )]
    async fn task_done(
        &self,
        Parameters(args): Parameters<TaskDoneArgs>,
    ) -> Result<Json<CompleteTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.complete_task_by_selector(
                &board,
                &args.task_ref,
                &CompleteTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                    force: args.force,
                    summary: args.summary,
                    result: args.result,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(CompleteTaskResponse::new(task)))
    }

    #[tool(
        name = "task_block",
        description = "Block an active task through the canonical application service"
    )]
    async fn task_block(
        &self,
        Parameters(args): Parameters<TaskBlockArgs>,
    ) -> Result<Json<BlockTaskResponse>, McpError> {
        let board = args.board.unwrap_or_else(|| self.default_board.to_string());
        let client = self.client.clone();
        let task = tokio::task::spawn_blocking(move || {
            client.block_task_by_selector(
                &board,
                &args.task_ref,
                &BlockTaskRequest {
                    actor: None,
                    reason: args.reason,
                    claim_token: args.claim_token,
                    force: args.force,
                },
            )
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?
        .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        Ok(Json(BlockTaskResponse::new(task)))
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
