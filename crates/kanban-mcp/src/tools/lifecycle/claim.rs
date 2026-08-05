use kanban_protocol::{ClaimTaskRequest, ClaimTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskClaimArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Claim lease duration in milliseconds.
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// Worker configuration recorded on the run. Defaults to manual.
    worker_profile: Option<String>,
    /// JSON metadata recorded on the run and claimed event.
    metadata: Option<serde_json::Value>,
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
}

#[tool_router(router = task_claim_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_claim",
        description = "Atomically claim a ready task and create its run through the canonical application service"
    )]
    async fn task_claim(
        &self,
        Parameters(args): Parameters<TaskClaimArgs>,
    ) -> Result<Json<ClaimTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let claim = call_client(move || {
            client.claim_task_by_selector(
                &board,
                &args.task_ref,
                &ClaimTaskRequest {
                    actor: None,
                    ttl_ms: args.ttl_ms,
                    worker_profile: args.worker_profile,
                    metadata: args.metadata,
                },
            )
        })
        .await?;
        Ok(Json(ClaimTaskResponse::new(claim)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_claim_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_claim_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_claim");
    }
}
