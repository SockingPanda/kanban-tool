use kanban_contract::{ReopenStepRequest, ReopenStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepReopenArgs {
    board: Option<String>,
    task_ref: String,
    step_ref: String,
    reason: String,
}

#[tool_router(router = step_reopen_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_reopen",
        description = "通过 canonical application service 重新打开 execution-plan step"
    )]
    async fn step_reopen(
        &self,
        Parameters(args): Parameters<StepReopenArgs>,
    ) -> Result<Json<ReopenStepResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let step_ref = args.step_ref;
        let client = self.client.clone();
        let before = {
            let client = client.clone();
            let board = board.clone();
            let task_ref = task_ref.clone();
            call_client(move || client.list_steps_by_selector(&board, &task_ref)).await?
        };
        let request = ReopenStepRequest {
            reason: args.reason,
            actor: None,
        };
        let step = call_client(move || {
            client.reopen_step_by_selector(&board, &task_ref, &step_ref, &request)
        })
        .await?;
        let mut steps = before;
        if let Some(found) = steps
            .steps
            .iter_mut()
            .find(|candidate| candidate.id == step.id)
        {
            *found = step;
        }
        Ok(Json(ReopenStepResponse { data: steps }))
    }
}
