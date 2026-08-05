use crate::shared::{KanbanMcp, call_client};
use kanban_contract::{ArchiveTaskRequest, ArchiveTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskArchiveArgs {
    board: Option<String>,
    task_ref: String,
    #[serde(default)]
    force: bool,
}
#[tool_router(router = task_archive_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "task_archive", description = "归档任务并保持 run/claim 一致")]
    async fn task_archive(
        &self,
        Parameters(args): Parameters<TaskArchiveArgs>,
    ) -> Result<Json<ArchiveTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.archive_task_by_selector(
                &board,
                &args.task_ref,
                &ArchiveTaskRequest {
                    actor: None,
                    force: args.force,
                },
            )
        })
        .await?;
        Ok(Json(ArchiveTaskResponse::new(task)))
    }
}
