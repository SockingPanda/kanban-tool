use kanban_protocol::{CreateStepRequest, CreateStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepCreateArgs {
    /// Board used when task_ref or linked_task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    title: String,
    body: Option<String>,
    linked_task_ref: Option<String>,
    position: Option<i64>,
    #[serde(default = "default_step_required")]
    required: bool,
    idempotency_key: Option<String>,
}

const fn default_step_required() -> bool {
    true
}

#[tool_router(router = step_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_create",
        description = "Create a todo execution-plan step through the canonical application service"
    )]
    async fn step_create(
        &self,
        Parameters(args): Parameters<StepCreateArgs>,
    ) -> Result<Json<CreateStepResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let request = CreateStepRequest {
            idempotency_key: args.idempotency_key,
            title: args.title,
            body: args.body,
            linked_task_ref: args.linked_task_ref,
            position: args.position,
            required: args.required,
            actor: None,
        };
        let steps =
            call_client(move || client.create_step_by_selector(&board, &task_ref, &request))
                .await?;
        Ok(Json(CreateStepResponse { data: steps }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn step_create_tool_is_independently_locatable() {
        let tools = KanbanMcp::step_create_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "step_create");
    }
}
