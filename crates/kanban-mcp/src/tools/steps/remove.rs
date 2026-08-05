use kanban_protocol::RemoveStepResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepRemoveArgs {
    board: Option<String>,
    task_ref: String,
    step_ref: String,
}

#[tool_router(router = step_remove_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_remove",
        description = "通过 canonical application service 删除 execution-plan step"
    )]
    async fn step_remove(
        &self,
        Parameters(args): Parameters<StepRemoveArgs>,
    ) -> Result<Json<RemoveStepResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let step_ref = args.step_ref;
        let client = self.client.clone();
        let steps =
            call_client(move || client.remove_step_by_selector(&board, &task_ref, &step_ref))
                .await?;
        Ok(Json(RemoveStepResponse { data: steps }))
    }
}
