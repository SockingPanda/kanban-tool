use kanban_contract::{SkipStepRequest, SkipStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepSkipArgs {
    board: Option<String>,
    task_ref: String,
    step_ref: String,
    reason: String,
}

#[tool_router(router = step_skip_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_skip",
        description = "通过 canonical application service 跳过 execution-plan step"
    )]
    async fn step_skip(
        &self,
        Parameters(args): Parameters<StepSkipArgs>,
    ) -> Result<Json<SkipStepResponse>, McpError> {
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
        let request = SkipStepRequest {
            reason: args.reason,
            actor: None,
        };
        let step = call_client(move || {
            client.skip_step_by_selector(&board, &task_ref, &step_ref, &request)
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
        Ok(Json(SkipStepResponse { data: steps }))
    }
}
