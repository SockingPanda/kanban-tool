use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

const MAX_TASK_LIST_LIMIT: usize = 1_000;
const MAX_TASK_QUERY_CHARS: usize = 1_024;
const MAX_TASK_ASSIGNEE_CHARS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanFilter {
    PlanNeeded,
    HasSteps,
    IncompleteRequiredSteps,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TaskListSort {
    Seq,
    SeqDesc,
    Title,
    TitleDesc,
    Status,
    StatusDesc,
    #[default]
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    Assignee,
    AssigneeDesc,
    ScheduledAt,
    ScheduledAtDesc,
    DueAt,
    DueAtDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<TaskStatus>,
    pub priorities: Vec<i64>,
    pub labels: Vec<String>,
    pub plan_filters: Vec<TaskPlanFilter>,
    pub assignee: Option<String>,
    pub query: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
    pub sort: TaskListSort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

pub trait TaskList: ApplicationStore {
    fn list_tasks(
        &self,
        board: &str,
        options: TaskListOptions,
    ) -> impl Future<Output = Result<TaskListPage>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskList,
    C: Clock,
{
    pub async fn list_tasks(
        &self,
        board: &str,
        mut options: TaskListOptions,
    ) -> Result<TaskListPage> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        if options.limit > MAX_TASK_LIST_LIMIT {
            return Err(KanbanError::InvalidInput(format!(
                "limit must be <= {MAX_TASK_LIST_LIMIT}"
            )));
        }
        if options
            .assignee
            .as_deref()
            .is_some_and(|value| value.chars().count() > MAX_TASK_ASSIGNEE_CHARS)
        {
            return Err(KanbanError::InvalidInput(format!(
                "assignee exceeds {MAX_TASK_ASSIGNEE_CHARS} characters"
            )));
        }
        if options
            .query
            .as_deref()
            .is_some_and(|value| value.chars().count() > MAX_TASK_QUERY_CHARS)
        {
            return Err(KanbanError::InvalidInput(format!(
                "query exceeds {MAX_TASK_QUERY_CHARS} characters"
            )));
        }
        if options
            .priorities
            .iter()
            .any(|value| !(0..=3).contains(value))
        {
            return Err(KanbanError::InvalidInput(
                "priority filters must be between 0 and 3".to_owned(),
            ));
        }
        options.assignee = trimmed_optional(options.assignee);
        options.query = trimmed_optional(options.query);
        options.labels = options
            .labels
            .into_iter()
            .map(|label| label.trim().to_owned())
            .filter(|label| !label.is_empty())
            .collect();
        self.store.list_tasks(board, options).await
    }
}

fn trimmed_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl TaskList for StubStore {
        async fn list_tasks(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage> {
            assert_eq!(board, "default");
            assert_eq!(options.assignee.as_deref(), Some("worker"));
            assert_eq!(options.query.as_deref(), Some("needle"));
            assert_eq!(options.labels, vec!["bug"]);
            Ok(TaskListPage {
                tasks: Vec::new(),
                total: 0,
            })
        }
    }
    #[tokio::test]
    async fn list_tasks_validates_and_normalizes_query_options() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let page = service
            .list_tasks(
                " default ",
                TaskListOptions {
                    statuses: vec![TaskStatus::Todo],
                    priorities: vec![1],
                    labels: vec![" bug ".into()],
                    plan_filters: Vec::new(),
                    assignee: Some(" worker ".into()),
                    query: Some(" needle ".into()),
                    include_archived: false,
                    limit: 25,
                    offset: 0,
                    sort: crate::TaskListSort::UpdatedAtDesc,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.total, 0);

        let error = service
            .list_tasks(
                "default",
                TaskListOptions {
                    statuses: Vec::new(),
                    priorities: Vec::new(),
                    labels: Vec::new(),
                    plan_filters: Vec::new(),
                    assignee: None,
                    query: None,
                    include_archived: false,
                    limit: 1_001,
                    offset: 0,
                    sort: crate::TaskListSort::default(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}
