//! 模块和迭代参数直接映射到共享 typed client。
use crate::shared::{KanbanMcp, call_client};
use kanban_protocol::*;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
use std::collections::BTreeMap;
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

fn default_planning_limit() -> usize {
    100
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListModulesArgs {
    board: Option<String>,

    q: Option<String>,
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_planning_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    parent_id: Option<String>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetModuleArgs {
    board: Option<String>,
    module_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateModuleArgs {
    board: Option<String>,

    title: String,
    body: Option<String>,
    parent_id: Option<String>,
    request_id: Option<String>,
    actor: Option<String>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateModuleArgs {
    board: Option<String>,
    module_id: String,

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
    body: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_nullable",
        skip_serializing_if = "Option::is_none"
    )]
    parent_id: Option<Option<String>>,
    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArchiveModuleArgs {
    board: Option<String>,
    module_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RestoreModuleArgs {
    board: Option<String>,
    module_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetModuleOverviewArgs {
    board: Option<String>,
    module_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListModuleTasksArgs {
    board: Option<String>,
    module_id: String,

    #[serde(default = "default_planning_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddModuleTaskArgs {
    board: Option<String>,
    module_id: String,
    task_ref: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveModuleTaskArgs {
    board: Option<String>,
    module_id: String,
    task_ref: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListCyclesArgs {
    board: Option<String>,

    q: Option<String>,
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_planning_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    status: Option<ApiCycleStatus>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetCycleArgs {
    board: Option<String>,
    cycle_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateCycleArgs {
    board: Option<String>,

    title: String,
    body: Option<String>,
    starts_at: i64,
    ends_at: i64,
    request_id: Option<String>,
    actor: Option<String>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateCycleArgs {
    board: Option<String>,
    cycle_id: String,

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
    body: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    starts_at: Option<i64>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_present",
        skip_serializing_if = "Option::is_none"
    )]
    ends_at: Option<i64>,
    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArchiveCycleArgs {
    board: Option<String>,
    cycle_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RestoreCycleArgs {
    board: Option<String>,
    cycle_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetCycleOverviewArgs {
    board: Option<String>,
    cycle_id: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListCycleTasksArgs {
    board: Option<String>,
    cycle_id: String,

    #[serde(default = "default_planning_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddCycleTaskArgs {
    board: Option<String>,
    cycle_id: String,
    task_ref: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveCycleTaskArgs {
    board: Option<String>,
    cycle_id: String,
    task_ref: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StartCycleArgs {
    board: Option<String>,
    cycle_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CloseCycleArgs {
    board: Option<String>,
    cycle_id: String,

    carry_to: Option<String>,
    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CancelCycleArgs {
    board: Option<String>,
    cycle_id: String,

    request_id: Option<String>,
    actor: Option<String>,
    expected_version: Option<i64>,
    #[serde(default)]
    expected_versions: BTreeMap<String, ApiObjectVersion>,
    expected_catalog_version: Option<i64>,
}
#[tool_router(router=planning_tools,vis="pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "module_list",
        description = "模块 list；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_list(
        &self,
        Parameters(args): Parameters<ListModulesArgs>,
    ) -> Result<Json<ListModulesResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.list_modules(
                &board,
                &ListModulesQuery {
                    q: args.q,
                    include_archived: args.include_archived,
                    limit: args.limit,
                    offset: args.offset,
                    parent_id: args.parent_id,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_show",
        description = "模块 show；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_show(
        &self,
        Parameters(args): Parameters<GetModuleArgs>,
    ) -> Result<Json<GetModuleResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.get_module(&board, &args.module_id)).await?,
        ))
    }
    #[tool(
        name = "module_create",
        description = "模块 create；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_create(
        &self,
        Parameters(args): Parameters<CreateModuleArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.create_module(
                &board,
                &CreateModuleRequest {
                    title: args.title,
                    body: args.body,
                    parent_id: args.parent_id,
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_update",
        description = "模块 update；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_update(
        &self,
        Parameters(args): Parameters<UpdateModuleArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.update_module(
                &board,
                &args.module_id,
                &UpdateModuleRequest {
                    title: args.title,
                    body: args.body,
                    parent_id: args.parent_id,
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_archive",
        description = "模块 archive；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_archive(
        &self,
        Parameters(args): Parameters<ArchiveModuleArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.archive_module(
                &board,
                &args.module_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_restore",
        description = "模块 restore；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_restore(
        &self,
        Parameters(args): Parameters<RestoreModuleArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.restore_module(
                &board,
                &args.module_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_overview",
        description = "模块 overview；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_overview(
        &self,
        Parameters(args): Parameters<GetModuleOverviewArgs>,
    ) -> Result<Json<ModuleOverviewResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.get_module_overview(&board, &args.module_id)).await?,
        ))
    }
    #[tool(
        name = "module_task_list",
        description = "模块 task list；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_task_list(
        &self,
        Parameters(args): Parameters<ListModuleTasksArgs>,
    ) -> Result<Json<PlanningMembersResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.list_module_tasks(
                &board,
                &args.module_id,
                &PlanningMembersQuery {
                    limit: args.limit,
                    offset: args.offset,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_task_add",
        description = "模块 task add；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_task_add(
        &self,
        Parameters(args): Parameters<AddModuleTaskArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.add_module_task(
                &board,
                &args.module_id,
                &args.task_ref,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "module_task_remove",
        description = "模块 task remove；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn module_task_remove(
        &self,
        Parameters(args): Parameters<RemoveModuleTaskArgs>,
    ) -> Result<Json<ModuleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.remove_module_task(
                &board,
                &args.module_id,
                &args.task_ref,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_list",
        description = "迭代 list；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_list(
        &self,
        Parameters(args): Parameters<ListCyclesArgs>,
    ) -> Result<Json<ListCyclesResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.list_cycles(
                &board,
                &ListCyclesQuery {
                    q: args.q,
                    include_archived: args.include_archived,
                    limit: args.limit,
                    offset: args.offset,
                    status: args.status,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_show",
        description = "迭代 show；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_show(
        &self,
        Parameters(args): Parameters<GetCycleArgs>,
    ) -> Result<Json<GetCycleResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.get_cycle(&board, &args.cycle_id)).await?,
        ))
    }
    #[tool(
        name = "cycle_create",
        description = "迭代 create；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_create(
        &self,
        Parameters(args): Parameters<CreateCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.create_cycle(
                &board,
                &CreateCycleRequest {
                    title: args.title,
                    body: args.body,
                    starts_at: args.starts_at,
                    ends_at: args.ends_at,
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_update",
        description = "迭代 update；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_update(
        &self,
        Parameters(args): Parameters<UpdateCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.update_cycle(
                &board,
                &args.cycle_id,
                &UpdateCycleRequest {
                    title: args.title,
                    body: args.body,
                    starts_at: args.starts_at,
                    ends_at: args.ends_at,
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_archive",
        description = "迭代 archive；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_archive(
        &self,
        Parameters(args): Parameters<ArchiveCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.archive_cycle(
                &board,
                &args.cycle_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_restore",
        description = "迭代 restore；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_restore(
        &self,
        Parameters(args): Parameters<RestoreCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.restore_cycle(
                &board,
                &args.cycle_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_overview",
        description = "迭代 overview；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_overview(
        &self,
        Parameters(args): Parameters<GetCycleOverviewArgs>,
    ) -> Result<Json<CycleOverviewResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.get_cycle_overview(&board, &args.cycle_id)).await?,
        ))
    }
    #[tool(
        name = "cycle_task_list",
        description = "迭代 task list；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_task_list(
        &self,
        Parameters(args): Parameters<ListCycleTasksArgs>,
    ) -> Result<Json<PlanningMembersResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.list_cycle_tasks(
                &board,
                &args.cycle_id,
                &PlanningMembersQuery {
                    limit: args.limit,
                    offset: args.offset,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_task_add",
        description = "迭代 task add；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_task_add(
        &self,
        Parameters(args): Parameters<AddCycleTaskArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.add_cycle_task(
                &board,
                &args.cycle_id,
                &args.task_ref,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_task_remove",
        description = "迭代 task remove；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_task_remove(
        &self,
        Parameters(args): Parameters<RemoveCycleTaskArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.remove_cycle_task(
                &board,
                &args.cycle_id,
                &args.task_ref,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_start",
        description = "迭代 start；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_start(
        &self,
        Parameters(args): Parameters<StartCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.start_cycle(
                &board,
                &args.cycle_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_close",
        description = "迭代 close；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_close(
        &self,
        Parameters(args): Parameters<CloseCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.close_cycle(
                &board,
                &args.cycle_id,
                &CloseCycleRequest {
                    carry_to: args.carry_to,
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
    #[tool(
        name = "cycle_cancel",
        description = "迭代 cancel；使用 canonical 对象关系，ID 必须为 obj_..."
    )]
    async fn cycle_cancel(
        &self,
        Parameters(args): Parameters<CancelCycleArgs>,
    ) -> Result<Json<CycleMutationResponse>, McpError> {
        let board = self.board(args.board);
        Ok(Json(
            call_client(self.client.cancel_cycle(
                &board,
                &args.cycle_id,
                &PlanningActionRequest {
                    request_id: args.request_id,
                    actor: args.actor,
                    expected_version: args.expected_version,
                    expected_versions: args.expected_versions,
                    expected_catalog_version: args.expected_catalog_version,
                },
            ))
            .await?,
        ))
    }
}
