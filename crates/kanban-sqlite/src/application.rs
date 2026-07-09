use std::path::{Path, PathBuf};

use kanban_application::api::{ApplicationBackend, EventRecord};
use kanban_application::dto::{
    ClaimResult, CreateTask, DispatchOptions, DispatchResult, EventListOptions, TaskListOptions,
    TaskListPage, TaskRecord,
};
use kanban_core::Result;

/// SQLite-backed implementation of the application use-case boundary.
#[derive(Debug, Clone)]
pub struct SqliteApplication {
    path: PathBuf,
}

impl SqliteApplication {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ApplicationBackend for SqliteApplication {
    fn create_task(&self, board: &str, actor: &str, input: CreateTask) -> Result<TaskRecord> {
        crate::service::create_task(&self.path, board, actor, input)
    }

    fn mark_execution_plan_not_required(
        &self,
        board: &str,
        actor: &str,
        task_ref: &str,
        reason: &str,
    ) -> Result<()> {
        crate::service::mark_execution_plan_not_required(&self.path, board, actor, task_ref, reason)
            .map(|_| ())
    }

    fn list_tasks_page(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage> {
        crate::service::list_tasks_page(&self.path, board, options)
    }

    fn claim_task(
        &self,
        board: &str,
        actor: &str,
        task_ref: &str,
        ttl_ms: i64,
    ) -> Result<ClaimResult> {
        crate::service::claim_task(&self.path, board, actor, task_ref, ttl_ms)
    }

    fn dispatch_once(&self, board: &str, options: DispatchOptions) -> Result<DispatchResult> {
        crate::service::dispatch_once(&self.path, board, options)
    }

    fn list_events_after(
        &self,
        board: &str,
        options: EventListOptions,
    ) -> Result<Vec<EventRecord>> {
        crate::service::list_events_after(&self.path, board, options)
    }
}
