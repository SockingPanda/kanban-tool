use crate::shared::{KanbanMcp, call_client};
use kanban_contract::{UpdateTaskRequest, UpdateTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

fn deserialize_patch_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn deserialize_patch_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskUpdateArgs {
    board: Option<String>,
    task_ref: String,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    title: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    assignee: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    priority: Option<i64>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    scheduled_at: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    due_at: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    max_retries: Option<Option<i64>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    metadata: Option<Option<serde_json::Value>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    expected_lock_version: Option<i64>,
}
#[tool_router(router = task_update_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_update",
        description = "更新任务安全字段并按 canonical 状态重算"
    )]
    async fn task_update(
        &self,
        Parameters(args): Parameters<TaskUpdateArgs>,
    ) -> Result<Json<UpdateTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.update_task_by_selector(
                &board,
                &args.task_ref,
                &UpdateTaskRequest {
                    title: args.title,
                    description: args.description,
                    assignee: args.assignee,
                    priority: args.priority,
                    scheduled_at: args.scheduled_at,
                    due_at: args.due_at,
                    max_retries: args.max_retries,
                    metadata: args.metadata,
                    actor: None,
                    expected_lock_version: args.expected_lock_version,
                },
            )
        })
        .await?;
        Ok(Json(UpdateTaskResponse::new(task)))
    }
}
