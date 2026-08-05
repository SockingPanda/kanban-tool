use kanban_protocol::{CompleteTaskRequest, CompleteTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

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

#[tool_router(router = task_done_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_done",
        description = "Complete a running or reviewed task through the canonical application service"
    )]
    async fn task_done(
        &self,
        Parameters(args): Parameters<TaskDoneArgs>,
    ) -> Result<Json<CompleteTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
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
        .await?;
        Ok(Json(CompleteTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_done_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_done_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_done");
    }
}
