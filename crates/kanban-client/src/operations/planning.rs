//! typed planning 调用及严格对象 ID/任务 selector 解析。
use crate::{ClientError, KanbanClient, transport::rpc};
use kanban_protocol::*;

impl KanbanClient {
    pub async fn planning_capabilities(&self) -> Result<PlanningCapabilities, ClientError> {
        let response: PlanningCapabilitiesResponse = rpc!(
            self,
            get_planning_capabilities,
            GetPlanningCapabilitiesRequest,
            (),
            (),
            (),
            capability_status
        )?;
        Ok(response.data)
    }
    /// 新字段需要 Host 明确认可；旧 Host 不会收到可能被忽略的写入或筛选。
    pub(crate) async fn require_planning(&self, filters: bool) -> Result<(), ClientError> {
        let c = self.planning_capabilities().await?;
        if c.version < 1
            || (if filters {
                !c.task_membership_filters
            } else {
                !c.task_membership
            })
        {
            return Err(ClientError::Api {
                status: 501,
                code: ApiErrorCode::FeatureNotAvailable,
                message: "当前 Host 未开放任务归属能力".into(),
            });
        }
        Ok(())
    }
    pub async fn list_modules(
        &self,
        board: &str,
        query: &ListModulesQuery,
    ) -> Result<ListModulesResponse, ClientError> {
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            list_modules,
            ListModulesRequest,
            PlanningBoardPath { board },
            query.clone(),
            ()
        )
    }
    pub async fn get_module(
        &self,
        board: &str,
        id: &str,
    ) -> Result<GetModuleResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            get_module,
            GetModuleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            ()
        )
    }
    pub async fn create_module(
        &self,
        board: &str,
        input: &CreateModuleRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            create_module,
            CreateModuleRequest,
            PlanningBoardPath { board },
            (),
            input
        )
    }
    pub async fn update_module(
        &self,
        board: &str,
        id: &str,
        input: &UpdateModuleRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            update_module,
            UpdateModuleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn archive_module(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            archive_module,
            ArchiveModuleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn restore_module(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            restore_module,
            RestoreModuleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn get_module_overview(
        &self,
        board: &str,
        id: &str,
    ) -> Result<ModuleOverviewResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            get_module_overview,
            GetModuleOverviewRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            ()
        )
    }
    pub async fn list_module_tasks(
        &self,
        board: &str,
        id: &str,
        query: &PlanningMembersQuery,
    ) -> Result<PlanningMembersResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            list_module_tasks,
            ListModuleTasksRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            query.clone(),
            ()
        )
    }
    pub async fn add_module_task(
        &self,
        board: &str,
        id: &str,
        task: &str,
        input: &PlanningActionRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let task_id = self.resolve_task_id(&board, task).await?;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            add_module_task,
            AddModuleTaskRequest,
            PlanningMemberPath {
                board,
                id: id.into(),
                task_id
            },
            (),
            input
        )
    }
    pub async fn remove_module_task(
        &self,
        board: &str,
        id: &str,
        task: &str,
        input: &PlanningActionRequest,
    ) -> Result<ModuleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let task_id = self.resolve_task_id(&board, task).await?;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            remove_module_task,
            RemoveModuleTaskRequest,
            PlanningMemberPath {
                board,
                id: id.into(),
                task_id
            },
            (),
            input
        )
    }
    pub async fn list_cycles(
        &self,
        board: &str,
        query: &ListCyclesQuery,
    ) -> Result<ListCyclesResponse, ClientError> {
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            list_cycles,
            ListCyclesRequest,
            PlanningBoardPath { board },
            query.clone(),
            ()
        )
    }
    pub async fn get_cycle(&self, board: &str, id: &str) -> Result<GetCycleResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            get_cycle,
            GetCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            ()
        )
    }
    pub async fn create_cycle(
        &self,
        board: &str,
        input: &CreateCycleRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            create_cycle,
            CreateCycleRequest,
            PlanningBoardPath { board },
            (),
            input
        )
    }
    pub async fn update_cycle(
        &self,
        board: &str,
        id: &str,
        input: &UpdateCycleRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            update_cycle,
            UpdateCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn archive_cycle(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            archive_cycle,
            ArchiveCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn restore_cycle(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            restore_cycle,
            RestoreCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn get_cycle_overview(
        &self,
        board: &str,
        id: &str,
    ) -> Result<CycleOverviewResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            get_cycle_overview,
            GetCycleOverviewRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            ()
        )
    }
    pub async fn list_cycle_tasks(
        &self,
        board: &str,
        id: &str,
        query: &PlanningMembersQuery,
    ) -> Result<PlanningMembersResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        rpc!(
            self,
            list_cycle_tasks,
            ListCycleTasksRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            query.clone(),
            ()
        )
    }
    pub async fn add_cycle_task(
        &self,
        board: &str,
        id: &str,
        task: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let task_id = self.resolve_task_id(&board, task).await?;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            add_cycle_task,
            AddCycleTaskRequest,
            PlanningMemberPath {
                board,
                id: id.into(),
                task_id
            },
            (),
            input
        )
    }
    pub async fn remove_cycle_task(
        &self,
        board: &str,
        id: &str,
        task: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let task_id = self.resolve_task_id(&board, task).await?;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            remove_cycle_task,
            RemoveCycleTaskRequest,
            PlanningMemberPath {
                board,
                id: id.into(),
                task_id
            },
            (),
            input
        )
    }
    pub async fn start_cycle(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            start_cycle,
            StartCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn close_cycle(
        &self,
        board: &str,
        id: &str,
        input: &CloseCycleRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            close_cycle,
            CloseCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
    pub async fn cancel_cycle(
        &self,
        board: &str,
        id: &str,
        input: &PlanningActionRequest,
    ) -> Result<CycleMutationResponse, ClientError> {
        object_id(id)?;
        let board = self.get_board(board).await?.id;
        let mut input = input.clone();
        input
            .request_id
            .get_or_insert_with(kanban_core::new_event_id);
        rpc!(
            self,
            cancel_cycle,
            CancelCycleRequest,
            PlanningObjectPath {
                board,
                id: id.into()
            },
            (),
            input
        )
    }
}
fn object_id(id: &str) -> Result<(), ClientError> {
    if !id.starts_with("obj_") || id.len() <= 4 {
        return Err(ClientError::InvalidInput(
            "模块和迭代必须使用 obj_... ID".into(),
        ));
    }
    Ok(())
}

fn capability_status(status: tonic::Status) -> ClientError {
    if status.code() == tonic::Code::Unimplemented {
        ClientError::Api {
            status: 501,
            code: ApiErrorCode::FeatureNotAvailable,
            message: "当前 Host 不支持模块/迭代任务归属；请升级 Host 后重试".into(),
        }
    } else {
        ClientError::status(status)
    }
}
