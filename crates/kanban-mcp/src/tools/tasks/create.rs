use std::collections::BTreeMap;

use kanban_contract::{ApiCreateTaskStatus, CreateTaskRequest, CreateTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

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
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

const fn default_priority() -> i64 {
    3
}

#[tool_router(router = task_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_create",
        description = "Create a task through the canonical kanban application service"
    )]
    async fn task_create(
        &self,
        Parameters(args): Parameters<TaskCreateArgs>,
    ) -> Result<Json<CreateTaskResponse>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let task = call_client(move || {
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
                    labels: args.labels,
                    depends_on: args.depends_on,
                    actor: None,
                },
            )
        })
        .await?;

        Ok(Json(CreateTaskResponse { data: task }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_create_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_create_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_create");
    }
}
