use kanban_protocol::{HeartbeatTaskRequest, HeartbeatTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskHeartbeatArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Exact token returned by task_claim.
    claim_token: String,
    /// New claim lease duration in milliseconds.
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// Optional heartbeat note recorded on the event.
    note: Option<String>,
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
}

#[tool_router(router = task_heartbeat_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_heartbeat",
        description = "Extend an active claim lease through the canonical application service"
    )]
    async fn task_heartbeat(
        &self,
        Parameters(args): Parameters<TaskHeartbeatArgs>,
    ) -> Result<Json<HeartbeatTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.heartbeat_task_by_selector(
                &board,
                &args.task_ref,
                &HeartbeatTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                    ttl_ms: args.ttl_ms,
                    note: args.note,
                },
            )
        })
        .await?;
        Ok(Json(HeartbeatTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_heartbeat_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_heartbeat_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_heartbeat");
    }
}
