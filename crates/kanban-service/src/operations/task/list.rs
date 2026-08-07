use kanban_core::{Clock, KanbanError, Result, TaskStatus};

use crate::{KanbanService, TaskRecord};

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

impl Default for TaskListOptions {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            priorities: Vec::new(),
            labels: Vec::new(),
            plan_filters: Vec::new(),
            assignee: None,
            query: None,
            include_archived: false,
            limit: 100,
            offset: 0,
            sort: TaskListSort::Position,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_tasks(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage> {
        let board = board.trim().to_owned();
        let options = normalize_task_list_options(&board, options)?;
        let store_options = crate::store_operations::StoreTaskListOptions {
            statuses: options
                .statuses
                .into_iter()
                .map(|status| status.as_str().to_owned())
                .collect(),
            priorities: options.priorities,
            labels: options.labels,
            include_archived: options.include_archived,
            assignee: options.assignee,
            q: options.query,
            plan_filters: options
                .plan_filters
                .into_iter()
                .map(store_plan_filter)
                .collect(),
            sort: store_task_sort(options.sort),
            limit: options.limit,
            offset: options.offset,
        };
        let page = self
            .store
            .list_tasks(&board, store_options)
            .await
            .map_err(crate::error::store_error)?;
        Ok(TaskListPage {
            tasks: page
                .tasks
                .into_iter()
                .map(super::application_task)
                .collect::<Result<Vec<_>>>()?,
            total: page.total,
        })
    }
}

fn normalize_task_list_options(
    board: &str,
    mut options: TaskListOptions,
) -> Result<TaskListOptions> {
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
    Ok(options)
}

fn trimmed_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn store_plan_filter(filter: TaskPlanFilter) -> crate::store_operations::StoreTaskPlanFilter {
    match filter {
        TaskPlanFilter::PlanNeeded => crate::store_operations::StoreTaskPlanFilter::PlanNeeded,
        TaskPlanFilter::HasSteps => crate::store_operations::StoreTaskPlanFilter::HasSteps,
        TaskPlanFilter::IncompleteRequiredSteps => {
            crate::store_operations::StoreTaskPlanFilter::IncompleteRequiredSteps
        }
    }
}

fn store_task_sort(sort: TaskListSort) -> crate::store_operations::StoreTaskListSort {
    use crate::store_operations::StoreTaskListSort as StoreSort;
    match sort {
        TaskListSort::Seq => StoreSort::Seq,
        TaskListSort::SeqDesc => StoreSort::SeqDesc,
        TaskListSort::Title => StoreSort::Title,
        TaskListSort::TitleDesc => StoreSort::TitleDesc,
        TaskListSort::Status => StoreSort::Status,
        TaskListSort::StatusDesc => StoreSort::StatusDesc,
        TaskListSort::Position => StoreSort::Position,
        TaskListSort::PositionDesc => StoreSort::PositionDesc,
        TaskListSort::Priority => StoreSort::Priority,
        TaskListSort::PriorityDesc => StoreSort::PriorityDesc,
        TaskListSort::Assignee => StoreSort::Assignee,
        TaskListSort::AssigneeDesc => StoreSort::AssigneeDesc,
        TaskListSort::ScheduledAt => StoreSort::ScheduledAt,
        TaskListSort::ScheduledAtDesc => StoreSort::ScheduledAtDesc,
        TaskListSort::DueAt => StoreSort::DueAt,
        TaskListSort::DueAtDesc => StoreSort::DueAtDesc,
        TaskListSort::CreatedAt => StoreSort::CreatedAt,
        TaskListSort::CreatedAtDesc => StoreSort::CreatedAtDesc,
        TaskListSort::UpdatedAt => StoreSort::UpdatedAt,
        TaskListSort::UpdatedAtDesc => StoreSort::UpdatedAtDesc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_core::{KanbanError, TaskStatus};

    #[test]
    fn list_options_normalize_query_filters() {
        let options = normalize_task_list_options(
            "default",
            TaskListOptions {
                statuses: vec![TaskStatus::Todo],
                priorities: vec![1],
                labels: vec![" bug ".into(), " ".into()],
                plan_filters: Vec::new(),
                assignee: Some(" worker ".into()),
                query: Some(" needle ".into()),
                include_archived: false,
                limit: 25,
                offset: 0,
                sort: TaskListSort::UpdatedAtDesc,
            },
        )
        .unwrap();
        assert_eq!(options.assignee.as_deref(), Some("worker"));
        assert_eq!(options.query.as_deref(), Some("needle"));
        assert_eq!(options.labels, vec!["bug"]);
    }

    #[test]
    fn list_options_reject_invalid_limit_and_priority() {
        let error = normalize_task_list_options(
            "default",
            TaskListOptions {
                limit: 1_001,
                statuses: Vec::new(),
                priorities: Vec::new(),
                labels: Vec::new(),
                plan_filters: Vec::new(),
                assignee: None,
                query: None,
                include_archived: false,
                offset: 0,
                sort: TaskListSort::default(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));

        let error = normalize_task_list_options(
            "default",
            TaskListOptions {
                statuses: Vec::new(),
                priorities: vec![4],
                labels: Vec::new(),
                plan_filters: Vec::new(),
                assignee: None,
                query: None,
                include_archived: false,
                limit: 100,
                offset: 0,
                sort: TaskListSort::default(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}
