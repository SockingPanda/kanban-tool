use kanban_contract::{SubmitReviewTaskRequest, SubmitReviewTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

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

#[tool_router(router = task_review_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_review",
        description = "Finish an active run and submit its task for review through the canonical application service"
    )]
    async fn task_review(
        &self,
        Parameters(args): Parameters<TaskReviewArgs>,
    ) -> Result<Json<SubmitReviewTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
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
        .await?;
        Ok(Json(SubmitReviewTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_review_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_review_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_review");
    }
}
