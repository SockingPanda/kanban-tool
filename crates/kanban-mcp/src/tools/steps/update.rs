use kanban_protocol::{UpdateStepRequest, UpdateStepResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepUpdateArgs {
    /// task_ref 或 linked_task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// 全局 step_... ID 或确定性的 S<n> 列表选择器。
    step_ref: String,
    title: Option<String>,
    /// 非 null body 会替换原 body；null 或省略表示保持不变。
    body: Option<String>,
    linked_task_ref: Option<String>,
    #[serde(default)]
    unlink_task: bool,
    position: Option<i64>,
    required: Option<bool>,
}

#[tool_router(router = step_update_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_update",
        description = "更新可编辑的 execution-plan 字段，不改变 step status"
    )]
    async fn step_update(
        &self,
        Parameters(args): Parameters<StepUpdateArgs>,
    ) -> Result<Json<UpdateStepResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let step_ref = args.step_ref;
        let client = self.client.clone();
        let request = UpdateStepRequest {
            title: args.title,
            body: args.body,
            linked_task_ref: args.linked_task_ref,
            unlink_task: args.unlink_task,
            position: args.position,
            required: args.required,
            actor: None,
        };
        let steps = call_client(move || {
            client.update_step_by_selector(&board, &task_ref, &step_ref, &request)
        })
        .await?;
        Ok(Json(UpdateStepResponse { data: steps }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn step_update_tool_is_independently_locatable() {
        let tools = KanbanMcp::step_update_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "step_update");
    }
}
