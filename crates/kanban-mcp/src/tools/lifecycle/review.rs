use kanban_protocol::{SubmitReviewTaskRequest, SubmitReviewTaskResponse};
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
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// task_claim 返回的 exact token。仅在 force 时可省略。
    claim_token: Option<String>,
    /// 绕过 caller credential checks，但不绕过 running-run consistency。
    #[serde(default)]
    force: bool,
    /// 可选 summary，会记录在 task 和 completed run 上。
    summary: Option<String>,
}

#[tool_router(router = task_review_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_review",
        description = "通过 canonical application service 结束 active run，并将其 task 提交 review"
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
