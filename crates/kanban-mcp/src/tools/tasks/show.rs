use kanban_contract::{GetTaskDetailsResponse, GetTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::Value;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskShowArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    #[serde(default)]
    include_details: bool,
}

#[tool_router(router = task_show_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_show",
        description = "Show one task through the canonical kanban application service"
    )]
    async fn task_show(
        &self,
        Parameters(args): Parameters<TaskShowArgs>,
    ) -> Result<Json<Value>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        if args.include_details {
            let detail =
                call_client(move || client.get_task_details_by_selector(&board, &args.task_ref))
                    .await?;
            return Ok(Json(
                serde_json::to_value(GetTaskDetailsResponse { data: detail })
                    .map_err(|error| McpError::internal_error(error.to_string(), None))?,
            ));
        }
        let task = call_client(move || client.get_task_by_selector(&board, &args.task_ref)).await?;
        Ok(Json(
            serde_json::to_value(GetTaskResponse::new(task, None))
                .map_err(|error| McpError::internal_error(error.to_string(), None))?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_show_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_show_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_show");
    }
}
