use kanban_protocol::{
    AddTaskLabelRequest, AddTaskLabelResponse, CreateBoardLabelRequest, CreateBoardLabelResponse,
    ListBoardLabelsResponse, ListTaskLabelsResponse, RemoveTaskLabelResponse,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct LabelListArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct LabelCreateArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskLabelListArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskLabelAddArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    name: Option<String>,
    names: Option<Vec<String>>,
    #[serde(default)]
    create_missing: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskLabelRemoveArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// Label ID 或完全匹配的 label name。
    label_id: String,
}

#[tool_router(router = label_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "label_list",
        description = "从 canonical kanban host 列出 board labels"
    )]
    async fn label_list(
        &self,
        Parameters(args): Parameters<LabelListArgs>,
    ) -> Result<Json<ListBoardLabelsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let labels = call_client(move || client.list_board_labels(&board)).await?;
        Ok(Json(ListBoardLabelsResponse { data: labels }))
    }

    #[tool(
        name = "label_create",
        description = "通过 canonical application service 创建 board label"
    )]
    async fn label_create(
        &self,
        Parameters(args): Parameters<LabelCreateArgs>,
    ) -> Result<Json<CreateBoardLabelResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let label = call_client(move || {
            client.create_board_label(
                &board,
                &CreateBoardLabelRequest {
                    name: args.name,
                    color: args.color,
                },
            )
        })
        .await?;
        Ok(Json(CreateBoardLabelResponse { data: label }))
    }

    #[tool(
        name = "task_label_list",
        description = "通过 canonical application service 列出任务上的 labels"
    )]
    async fn task_label_list(
        &self,
        Parameters(args): Parameters<TaskLabelListArgs>,
    ) -> Result<Json<ListTaskLabelsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let labels =
            call_client(move || client.list_task_labels_by_selector(&board, &task_ref)).await?;
        Ok(Json(ListTaskLabelsResponse { data: labels }))
    }

    #[tool(
        name = "task_label_add",
        description = "通过 canonical application service 为任务附加一个或多个 labels"
    )]
    async fn task_label_add(
        &self,
        Parameters(args): Parameters<TaskLabelAddArgs>,
    ) -> Result<Json<AddTaskLabelResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let request = AddTaskLabelRequest {
            name: args.name,
            names: args.names,
            create_missing: args.create_missing,
            actor: None,
        };
        let response =
            call_client(move || client.add_task_labels_by_selector(&board, &task_ref, &request))
                .await?;
        Ok(Json(response))
    }

    #[tool(
        name = "task_label_remove",
        description = "通过 canonical application service 删除任务 label"
    )]
    async fn task_label_remove(
        &self,
        Parameters(args): Parameters<TaskLabelRemoveArgs>,
    ) -> Result<Json<RemoveTaskLabelResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let label_id = args.label_id;
        let client = self.client.clone();
        let task =
            call_client(move || client.remove_task_label_by_selector(&board, &task_ref, &label_id))
                .await?;
        Ok(Json(RemoveTaskLabelResponse { data: task }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn label_tools_are_independently_locatable() {
        let names: Vec<_> = KanbanMcp::label_tools()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "label_create",
                "label_list",
                "task_label_add",
                "task_label_list",
                "task_label_remove",
            ]
        );
    }
}
