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
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// claim lease 时长，单位为毫秒。
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    /// 记录在 run 上的 worker configuration。默认值为 manual。
    worker_profile: Option<String>,
    /// 记录在 run 和 claimed event 上的 JSON metadata。
    metadata: Option<serde_json::Value>,
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
}

#[tool_router(router = task_claim_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_claim",
        description = "通过 canonical application service 原子 claim ready task 并创建其 run"
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
