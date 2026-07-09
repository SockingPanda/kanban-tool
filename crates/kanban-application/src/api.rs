//! Public application API boundary for selected use-case slices.
//!
//! Functions in this module are thin dispatch helpers over `ApplicationBackend`.
//! They define the currently selected adapter contract; they do not imply that
//! every `kanban-sqlite::service` use case has moved into this crate.

use kanban_core::Result;

pub use crate::dto::*;

/// Backend implementation for the selected application use-case slices.
///
/// Add methods here only when an adapter-facing use case is intentionally moved
/// behind the application boundary. Prefer additive evolution or extension
/// traits over changing existing signatures in place.
pub trait ApplicationBackend {
    fn create_task(&self, board: &str, actor: &str, input: CreateTask) -> Result<TaskRecord>;

    fn mark_execution_plan_not_required(
        &self,
        board: &str,
        actor: &str,
        task_ref: &str,
        reason: &str,
    ) -> Result<()>;

    fn list_tasks_page(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage>;

    fn claim_task(
        &self,
        board: &str,
        actor: &str,
        task_ref: &str,
        ttl_ms: i64,
    ) -> Result<ClaimResult>;

    fn dispatch_once(&self, board: &str, options: DispatchOptions) -> Result<DispatchResult>;

    fn list_events_after(&self, board: &str, options: EventListOptions)
    -> Result<Vec<EventRecord>>;
}

pub fn create_task(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    actor: &str,
    input: CreateTask,
) -> Result<TaskRecord> {
    backend.create_task(board, actor, input)
}

pub fn mark_execution_plan_not_required(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    actor: &str,
    task_ref: &str,
    reason: &str,
) -> Result<()> {
    backend.mark_execution_plan_not_required(board, actor, task_ref, reason)
}

pub fn list_tasks_page(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    options: TaskListOptions,
) -> Result<TaskListPage> {
    backend.list_tasks_page(board, options)
}

pub fn claim_task(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    backend.claim_task(board, actor, task_ref, ttl_ms)
}

pub fn dispatch_once(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    options: DispatchOptions,
) -> Result<DispatchResult> {
    backend.dispatch_once(board, options)
}

pub fn list_events_after(
    backend: &(impl ApplicationBackend + ?Sized),
    board: &str,
    options: EventListOptions,
) -> Result<Vec<EventRecord>> {
    backend.list_events_after(board, options)
}
