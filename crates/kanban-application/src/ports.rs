use std::future::Future;

use kanban_core::Result;

use crate::{
    BlockTaskRecord, BoardColumnRecord, BoardRecord, ClaimRecord, ClaimTaskRecord,
    CompleteTaskRecord, CreateTaskRecord, ExecutionPlanRecord, HeartbeatTaskRecord,
    MarkExecutionPlanNotRequiredRecord, PromoteTaskRecord, ReleaseTaskRecord,
    SubmitReviewTaskRecord, TaskListOptions, TaskListPage, TaskRecord,
};

/// Persistence port used only by the shared application service.
///
/// The concrete Turso implementation is adapted inside `kanban-server`, which
/// keeps the storage crate out of every other product adapter.
pub trait ApplicationStore: Clone + Send + Sync + 'static {
    fn list_boards(
        &self,
        include_archived: bool,
    ) -> impl Future<Output = Result<Vec<BoardRecord>>> + Send;

    fn list_board_columns(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<Vec<BoardColumnRecord>>> + Send;

    fn create_task(
        &self,
        board: &str,
        input: CreateTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn list_tasks(
        &self,
        board: &str,
        options: TaskListOptions,
    ) -> impl Future<Output = Result<TaskListPage>> + Send;

    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: MarkExecutionPlanNotRequiredRecord,
    ) -> impl Future<Output = Result<ExecutionPlanRecord>> + Send;

    fn promote_task(
        &self,
        task_id: &str,
        input: PromoteTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn claim_task(
        &self,
        task_id: &str,
        input: ClaimTaskRecord,
    ) -> impl Future<Output = Result<ClaimRecord>> + Send;

    fn heartbeat_task(
        &self,
        task_id: &str,
        input: HeartbeatTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn release_task(
        &self,
        task_id: &str,
        input: ReleaseTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn submit_review_task(
        &self,
        task_id: &str,
        input: SubmitReviewTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn complete_task(
        &self,
        task_id: &str,
        input: CompleteTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn block_task(
        &self,
        task_id: &str,
        input: BlockTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}
