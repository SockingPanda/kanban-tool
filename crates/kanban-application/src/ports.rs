use std::future::Future;

use kanban_core::Result;

use crate::{
    AddDependencyRecord, AddDependencyResult, BlockTaskRecord, BoardColumnRecord, BoardRecord,
    ClaimRecord, ClaimTaskRecord, CommentRecord, CompleteTaskRecord, CreateCommentRecord,
    CreateStepRecord, CreateTaskRecord, DependencySnapshotRecord, ExecutionPlanRecord,
    HeartbeatTaskRecord, MarkExecutionPlanNotRequiredRecord, PromoteTaskRecord,
    ReclaimExpiredTaskRecord, ReleaseTaskRecord, RemoveDependencyResult, StepRecord,
    SubmitReviewTaskRecord, TaskListOptions, TaskListPage, TaskRecord, TaskStepsRecord,
    UpdateStepRecord,
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

    fn list_expired_claims(
        &self,
        board: &str,
        now: i64,
    ) -> impl Future<Output = Result<Vec<TaskRecord>>> + Send;

    fn reclaim_expired_task(
        &self,
        task_id: &str,
        input: ReclaimExpiredTaskRecord,
    ) -> impl Future<Output = Result<Option<TaskRecord>>> + Send;

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

    fn create_comment(
        &self,
        task_id: &str,
        input: CreateCommentRecord,
    ) -> impl Future<Output = Result<CommentRecord>> + Send;

    fn list_comments(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Vec<CommentRecord>>> + Send;

    fn add_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: AddDependencyRecord,
    ) -> impl Future<Output = Result<AddDependencyResult>> + Send;

    fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        actor: String,
        event_id: String,
        now: i64,
    ) -> impl Future<Output = Result<RemoveDependencyResult>> + Send;

    fn list_dependencies(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<DependencySnapshotRecord>> + Send;

    fn create_step(
        &self,
        task_id: &str,
        input: CreateStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;

    fn update_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: UpdateStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;
}
