mod archive;
mod block;
mod claim;
mod create;
mod details;
mod done;
mod heartbeat;
mod list;
mod plan_not_required;
mod promote;
mod reclaim;
mod release;
mod reopen;
mod review;
mod show;
mod specify;
mod unblock;
mod update;

pub use archive::{ArchiveTaskCommand, ArchiveTaskRecord, TaskArchive};
pub use block::{BlockTaskCommand, BlockTaskRecord, TaskBlock};
pub use claim::{ClaimTaskCommand, ClaimTaskRecord, TaskClaim};
pub use create::CreateTaskCommand;
pub use details::{
    TaskDetailOntologyRecord, TaskDetailRead, TaskDetailRecord, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord,
};
pub use done::{CompleteTaskCommand, CompleteTaskRecord, TaskDone};
pub use heartbeat::{HeartbeatTaskCommand, HeartbeatTaskRecord, TaskHeartbeat};
pub use list::{TaskListOptions, TaskListPage, TaskListSort, TaskPlanFilter};
pub use plan_not_required::{
    MarkExecutionPlanNotRequiredCommand, MarkExecutionPlanNotRequiredRecord, TaskPlanNotRequired,
};
pub use promote::{PromoteTaskCommand, PromoteTaskRecord, TaskPromote};
pub use reclaim::{
    ReclaimExpiredTaskRecord, ReclaimTaskCommand, ReclaimTaskRecord, TaskReclaim,
    TaskReclaimExplicit,
};
pub use release::{ReleaseTaskCommand, ReleaseTaskRecord, TaskRelease};
pub use reopen::{ReopenTaskCommand, ReopenTaskRecord, TaskReopen};
pub use review::{SubmitReviewTaskCommand, SubmitReviewTaskRecord, TaskReview};
pub use specify::{SpecifyTaskCommand, SpecifyTaskRecord, TaskSpecify};
pub use unblock::{TaskUnblock, UnblockTaskCommand, UnblockTaskRecord};
pub use update::UpdateTaskCommand;

/// 将 Turso 的任务 row 转为 application DTO。
///
/// row 类型只存在于 service 内；所有 task operation 和仍保留的 lifecycle adapter
/// 都通过这个 helper 共享同一套状态与执行计划校验。
pub(crate) fn application_task(
    task: crate::domain::TaskRecord,
) -> crate::Result<crate::TaskRecord> {
    let execution_plan_state = match task.execution_plan_state.as_str() {
        "unplanned" => crate::ExecutionPlanState::Unplanned,
        "planned" => crate::ExecutionPlanState::Planned,
        "not_required" => crate::ExecutionPlanState::NotRequired,
        other => {
            return Err(crate::KanbanError::Storage(format!(
                "stored execution plan state is invalid: {other}"
            )));
        }
    };
    Ok(crate::TaskRecord {
        id: task.id,
        board_id: task.board_id,
        board_slug: task.board_slug,
        task_ref: task.task_ref,
        seq: task.seq,
        title: task.title,
        description: task.description,
        status: task.status.parse::<kanban_core::TaskStatus>()?,
        status_reason: task.status_reason,
        assignee: task.assignee,
        priority: task.priority,
        position: task.position,
        scheduled_at: task.scheduled_at,
        due_at: task.due_at,
        created_by: task.created_by,
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        archived_at: task.archived_at,
        has_claim_token: task.claim_token.is_some(),
        claim_owner: task.claim_owner,
        claim_expires_at: task.claim_expires_at,
        last_heartbeat_at: task.last_heartbeat_at,
        current_run_id: task.current_run_id,
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        result_summary: task.result_summary,
        result_json: task.result_json,
        metadata_json: task.metadata_json,
        lock_version: task.lock_version,
        dependency_blocked: task.dependency_blocked,
        unfinished_parent_count: task.unfinished_parent_count,
        execution_plan_state,
        required_step_count: task.required_step_count,
        completed_required_step_count: task.completed_required_step_count,
        optional_step_count: task.optional_step_count,
        labels: task
            .labels
            .into_iter()
            .map(crate::adapter::application_label)
            .collect(),
    })
}
