use kanban_contract::{CompleteStepRequest, CompleteStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepDoneArgs {
    board: Option<String>,
    task_ref: String,
    step_ref: String,
    note: String,
}

#[tool_router(router = step_done_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_done",
        description = "通过 canonical application service 将 execution-plan step 标记为完成"
    )]
    async fn step_done(
        &self,
        Parameters(args): Parameters<StepDoneArgs>,
    ) -> Result<Json<CompleteStepResponse>, McpError> {
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
        let request = CompleteStepRequest {
            note: args.note,
            actor: None,
        };
        let step = call_client(move || {
            client.complete_step_by_selector(&board, &task_ref, &step_ref, &request)
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
        Ok(Json(CompleteStepResponse { data: steps }))
    }
}
