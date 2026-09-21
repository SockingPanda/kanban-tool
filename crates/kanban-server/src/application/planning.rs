//! 具名 planning RPC 的薄适配层；事务与对象约束由 service 持有。
use crate::{
    AppState,
    application::support::{CallContext, request_actor},
    error::ApiError,
};
use kanban_protocol::*;
use kanban_service::KanbanError;
use kanban_service::object_model::{self as object, planning as app};
use std::collections::BTreeMap;

fn error(e: object::ObjectError) -> ApiError {
    use object::ObjectErrorCode as C;
    match e.code {
        C::InvalidArgument => KanbanError::InvalidInput(e.message),
        C::NotFound => KanbanError::NotFound(e.message),
        C::Conflict => KanbanError::Conflict(e.message),
        C::FailedPrecondition => KanbanError::InvalidTransition(e.message),
        C::Storage | C::CommitUnknown => KanbanError::Storage(e.message),
    }
    .into()
}
pub(crate) fn versions(
    values: BTreeMap<String, ApiObjectVersion>,
) -> BTreeMap<String, object::ObjectVersion> {
    values
        .into_iter()
        .map(|(id, v)| {
            (
                id,
                object::ObjectVersion {
                    object: v.object,
                    source: v.source,
                },
            )
        })
        .collect()
}
fn receipt(r: object::ObjectReceipt) -> ApiPlanningReceipt {
    ApiPlanningReceipt {
        request_id: r.request_id,
        ids: r.ids,
        event_sequence: r.event_sequence,
        catalog_version: r.catalog_version,
        replayed: r.replayed,
        versions: r
            .versions
            .into_iter()
            .map(|(id, v)| {
                (
                    id,
                    ApiObjectVersion {
                        object: v.object,
                        source: v.source,
                    },
                )
            })
            .collect(),
    }
}
fn source(frozen: bool) -> PlanningReadSource {
    if frozen {
        PlanningReadSource::FrozenSnapshot
    } else {
        PlanningReadSource::Current
    }
}
fn progress(v: object::Progress) -> ApiPlanningProgress {
    ApiPlanningProgress {
        total: v.total,
        done: v.done,
        archived: v.archived,
        blocked: v.blocked,
        completion_ratio: v.completion_ratio,
    }
}
fn cycle_status(v: &str) -> Result<ApiCycleStatus, ApiError> {
    match v {
        "planned" => Ok(ApiCycleStatus::Planned),
        "active" => Ok(ApiCycleStatus::Active),
        "completed" => Ok(ApiCycleStatus::Completed),
        "cancelled" => Ok(ApiCycleStatus::Cancelled),
        _ => Err(KanbanError::Storage("迭代状态无效".into()).into()),
    }
}
fn state_name(v: ApiCycleStatus) -> String {
    match v {
        ApiCycleStatus::Planned => "planned",
        ApiCycleStatus::Active => "active",
        ApiCycleStatus::Completed => "completed",
        ApiCycleStatus::Cancelled => "cancelled",
    }
    .into()
}
fn module(v: app::PlanningRecord) -> Result<ApiModule, ApiError> {
    Ok(ApiModule {
        id: v.id,
        board_id: v.board_id,
        title: v.title,
        version: v.version.object,
        created_at: v.created_at,
        updated_at: v.updated_at,
        archived_at: v.archived_at,
        body: v.body,
        parent_id: v.parent_id,
    })
}
fn module_summary(v: app::PlanningRecord) -> Result<ApiModuleSummary, ApiError> {
    Ok(ApiModuleSummary {
        id: v.id,
        board_id: v.board_id,
        title: v.title,
        version: v.version.object,
        created_at: v.created_at,
        updated_at: v.updated_at,
        archived_at: v.archived_at,
        parent_id: v.parent_id,
    })
}
fn cycle(v: app::PlanningRecord) -> Result<ApiCycle, ApiError> {
    Ok(ApiCycle {
        id: v.id,
        board_id: v.board_id,
        title: v.title,
        version: v.version.object,
        created_at: v.created_at,
        updated_at: v.updated_at,
        archived_at: v.archived_at,
        body: v.body,
        status: cycle_status(
            v.status
                .as_deref()
                .ok_or_else(|| KanbanError::Storage("缺少迭代状态".into()))?,
        )?,
        starts_at: v
            .starts_at
            .ok_or_else(|| KanbanError::Storage("缺少开始时间".into()))?,
        ends_at: v
            .ends_at
            .ok_or_else(|| KanbanError::Storage("缺少结束时间".into()))?,
        started_at: v.started_at,
        closed_at: v.closed_at,
    })
}
fn cycle_summary(v: app::PlanningRecord) -> Result<ApiCycleSummary, ApiError> {
    Ok(ApiCycleSummary {
        id: v.id,
        board_id: v.board_id,
        title: v.title,
        version: v.version.object,
        created_at: v.created_at,
        updated_at: v.updated_at,
        archived_at: v.archived_at,
        status: cycle_status(
            v.status
                .as_deref()
                .ok_or_else(|| KanbanError::Storage("缺少迭代状态".into()))?,
        )?,
        starts_at: v
            .starts_at
            .ok_or_else(|| KanbanError::Storage("缺少开始时间".into()))?,
        ends_at: v
            .ends_at
            .ok_or_else(|| KanbanError::Storage("缺少结束时间".into()))?,
        started_at: v.started_at,
        closed_at: v.closed_at,
    })
}
pub(crate) async fn get_planning_capabilities(
    _state: AppState,
) -> Result<PlanningCapabilitiesResponse, ApiError> {
    Ok(DataEnvelope::new(PlanningCapabilities {
        version: 1,
        task_membership: true,
        task_membership_filters: true,
    }))
}
pub(crate) async fn list_modules(
    state: AppState,
    path: PlanningBoardPath,
    query: ListModulesQuery,
) -> Result<ListModulesResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_list(
            &board.id,
            app::PlanningKind::Module,
            app::PlanningListOptions {
                q: query.q,
                include_archived: query.include_archived,
                limit: query.limit,
                offset: query.offset,
                parent_id: query.parent_id,
                status: None,
            },
        )
        .await
        .map_err(error)?;
    Ok(ListModulesResponse {
        data: result
            .items
            .into_iter()
            .map(module_summary)
            .collect::<Result<_, _>>()?,
        total: result.total,
        limit: query.limit,
        offset: query.offset,
    })
}
pub(crate) async fn get_module(
    state: AppState,
    path: PlanningObjectPath,
) -> Result<GetModuleResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    Ok(DataEnvelope::new(module(
        state
            .application()
            .planning_get(&board.id, app::PlanningKind::Module, &path.id)
            .await
            .map_err(error)?,
    )?))
}
pub(crate) async fn create_module(
    state: AppState,
    path: PlanningBoardPath,
    context: CallContext,
    input: CreateModuleRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: None,
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Create {
                title: input.title,
                body: input.body,
                parent_id: input.parent_id,
                starts_at: None,
                ends_at: None,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn update_module(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: UpdateModuleRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Update {
                id: path.id,
                title: input.title,
                body: input.body,
                parent_id: input.parent_id,
                starts_at: None,
                ends_at: None,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn archive_module(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Archive { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn restore_module(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Restore { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn get_module_overview(
    state: AppState,
    path: PlanningObjectPath,
) -> Result<ModuleOverviewResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_overview(&board.id, app::PlanningKind::Module, &path.id)
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleOverview {
        object: module(result.object)?,
        progress: progress(result.progress),
        source: source(result.frozen),
        captured_at: result.captured_at,
    }))
}
pub(crate) async fn list_module_tasks(
    state: AppState,
    path: PlanningObjectPath,
    query: PlanningMembersQuery,
) -> Result<PlanningMembersResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_members(
            &board.id,
            app::PlanningKind::Module,
            &path.id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(error)?;
    Ok(PlanningMembersResponse {
        data: result
            .items
            .into_iter()
            .map(|m| {
                Ok(ApiPlanningMember {
                    id: m.id,
                    title: m.title,
                    status: super::tasks::support::api_task_status(m.status.parse()?),
                    version: ApiObjectVersion {
                        object: m.version.object,
                        source: m.version.source,
                    },
                    carried_to: m.carried_to,
                })
            })
            .collect::<Result<_, ApiError>>()?,
        total: result.total,
        limit: query.limit,
        offset: query.offset,
        source: source(result.frozen),
        captured_at: result.captured_at,
        object_version: result.version.object,
    })
}
pub(crate) async fn add_module_task(
    state: AppState,
    path: PlanningMemberPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::TaskAdd {
                id: path.id,
                task_id: path.task_id,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn remove_module_task(
    state: AppState,
    path: PlanningMemberPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<ModuleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Module,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::TaskRemove {
                id: path.id,
                task_id: path.task_id,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(ModuleMutationData {
        object: module(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn list_cycles(
    state: AppState,
    path: PlanningBoardPath,
    query: ListCyclesQuery,
) -> Result<ListCyclesResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_list(
            &board.id,
            app::PlanningKind::Cycle,
            app::PlanningListOptions {
                q: query.q,
                include_archived: query.include_archived,
                limit: query.limit,
                offset: query.offset,
                parent_id: None,
                status: query.status.map(state_name),
            },
        )
        .await
        .map_err(error)?;
    Ok(ListCyclesResponse {
        data: result
            .items
            .into_iter()
            .map(cycle_summary)
            .collect::<Result<_, _>>()?,
        total: result.total,
        limit: query.limit,
        offset: query.offset,
    })
}
pub(crate) async fn get_cycle(
    state: AppState,
    path: PlanningObjectPath,
) -> Result<GetCycleResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    Ok(DataEnvelope::new(cycle(
        state
            .application()
            .planning_get(&board.id, app::PlanningKind::Cycle, &path.id)
            .await
            .map_err(error)?,
    )?))
}
pub(crate) async fn create_cycle(
    state: AppState,
    path: PlanningBoardPath,
    context: CallContext,
    input: CreateCycleRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: None,
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Create {
                title: input.title,
                body: input.body,
                parent_id: None,
                starts_at: Some(input.starts_at),
                ends_at: Some(input.ends_at),
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn update_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: UpdateCycleRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Update {
                id: path.id,
                title: input.title,
                body: input.body,
                parent_id: None,
                starts_at: input.starts_at,
                ends_at: input.ends_at,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn archive_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Archive { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn restore_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Restore { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn get_cycle_overview(
    state: AppState,
    path: PlanningObjectPath,
) -> Result<CycleOverviewResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_overview(&board.id, app::PlanningKind::Cycle, &path.id)
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleOverview {
        object: cycle(result.object)?,
        progress: progress(result.progress),
        source: source(result.frozen),
        captured_at: result.captured_at,
    }))
}
pub(crate) async fn list_cycle_tasks(
    state: AppState,
    path: PlanningObjectPath,
    query: PlanningMembersQuery,
) -> Result<PlanningMembersResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let result = state
        .application()
        .planning_members(
            &board.id,
            app::PlanningKind::Cycle,
            &path.id,
            query.limit,
            query.offset,
        )
        .await
        .map_err(error)?;
    Ok(PlanningMembersResponse {
        data: result
            .items
            .into_iter()
            .map(|m| {
                Ok(ApiPlanningMember {
                    id: m.id,
                    title: m.title,
                    status: super::tasks::support::api_task_status(m.status.parse()?),
                    version: ApiObjectVersion {
                        object: m.version.object,
                        source: m.version.source,
                    },
                    carried_to: m.carried_to,
                })
            })
            .collect::<Result<_, ApiError>>()?,
        total: result.total,
        limit: query.limit,
        offset: query.offset,
        source: source(result.frozen),
        captured_at: result.captured_at,
        object_version: result.version.object,
    })
}
pub(crate) async fn add_cycle_task(
    state: AppState,
    path: PlanningMemberPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::TaskAdd {
                id: path.id,
                task_id: path.task_id,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn remove_cycle_task(
    state: AppState,
    path: PlanningMemberPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::TaskRemove {
                id: path.id,
                task_id: path.task_id,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn start_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Start { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn close_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: CloseCycleRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Close {
                id: path.id,
                carry_to: input.carry_to,
            },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
pub(crate) async fn cancel_cycle(
    state: AppState,
    path: PlanningObjectPath,
    context: CallContext,
    input: PlanningActionRequest,
) -> Result<CycleMutationResponse, ApiError> {
    let board = state.application().get_board(&path.board).await?;
    let actor = request_actor(input.actor.as_deref(), &context, state.default_actor())?;
    let result = state
        .application()
        .planning_execute(app::PlanningCommand {
            board_id: board.id,
            kind: app::PlanningKind::Cycle,
            actor,
            request_id: input.request_id.unwrap_or_else(kanban_service::new_task_id),
            versions: app::PlanningVersions {
                expected_version: input.expected_version.map(|object| object::ObjectVersion {
                    object,
                    source: None,
                }),
                expected_versions: versions(input.expected_versions),
                expected_catalog_version: input.expected_catalog_version,
            },
            mutation: app::PlanningMutation::Cancel { id: path.id },
        })
        .await
        .map_err(error)?;
    Ok(DataEnvelope::new(CycleMutationData {
        object: cycle(result.object)?,
        receipt: receipt(result.receipt),
    }))
}
