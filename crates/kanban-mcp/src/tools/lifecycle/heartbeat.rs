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
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// task_claim 返回的 exact token。
    claim_token: String,
    /// 新的 claim lease 时长，单位为毫秒。
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// 可选 heartbeat note，会记录在 event 上。
    note: Option<String>,
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
}

#[tool_router(router = task_heartbeat_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_heartbeat",
        description = "通过 canonical application service 延长 active claim lease"
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
