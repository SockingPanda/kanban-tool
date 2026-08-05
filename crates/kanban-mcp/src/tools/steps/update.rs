use kanban_protocol::{UpdateStepRequest, UpdateStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepUpdateArgs {
    /// Board used when task_ref or linked_task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Global step_... id or deterministic S<n> list selector.
    step_ref: String,
    title: Option<String>,
    /// A non-null body replaces the body; null/omitted leaves it unchanged.
    body: Option<String>,
    linked_task_ref: Option<String>,
    #[serde(default)]
    unlink_task: bool,
    position: Option<i64>,
    required: Option<bool>,
}

#[tool_router(router = step_update_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_update",
        description = "Update editable execution-plan fields without changing step status"
    )]
    async fn step_update(
        &self,
        Parameters(args): Parameters<StepUpdateArgs>,
    ) -> Result<Json<UpdateStepResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let step_ref = args.step_ref;
        let client = self.client.clone();
        let request = UpdateStepRequest {
            title: args.title,
            body: args.body,
            linked_task_ref: args.linked_task_ref,
            unlink_task: args.unlink_task,
            position: args.position,
            required: args.required,
            actor: None,
        };
        let steps = call_client(move || {
            client.update_step_by_selector(&board, &task_ref, &step_ref, &request)
        })
        .await?;
        Ok(Json(UpdateStepResponse { data: steps }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn step_update_tool_is_independently_locatable() {
        let tools = KanbanMcp::step_update_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "step_update");
    }
}
