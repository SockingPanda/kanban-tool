use crate::error::ApiError;
use kanban_protocol::{
    ApiTaskStatus, ListTasksByStatusQuery, ListTasksQuery, MAX_TASK_READ_ASSIGNEE_CHARS,
    MAX_TASK_READ_LABEL_CHARS, MAX_TASK_READ_LABELS, MAX_TASK_READ_LIMIT,
    MAX_TASK_READ_PLAN_FILTERS, MAX_TASK_READ_PRIORITIES, MAX_TASK_READ_Q_CHARS,
    MAX_TASK_READ_STATUSES, TaskReadPlanFilter, TaskReadSort,
};
use kanban_service::{
    KanbanError, TaskListSort as ApplicationTaskListSort,
    TaskPlanFilter as ApplicationTaskPlanFilter, TaskStatus,
};
pub(super) fn task_status(status: ApiTaskStatus) -> TaskStatus {
    match status {
        ApiTaskStatus::Triage => TaskStatus::Triage,
        ApiTaskStatus::Todo => TaskStatus::Todo,
        ApiTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiTaskStatus::Ready => TaskStatus::Ready,
        ApiTaskStatus::Running => TaskStatus::Running,
        ApiTaskStatus::Blocked => TaskStatus::Blocked,
        ApiTaskStatus::Review => TaskStatus::Review,
        ApiTaskStatus::Done => TaskStatus::Done,
        ApiTaskStatus::Archived => TaskStatus::Archived,
    }
}
pub(super) fn application_plan_filter(filter: TaskReadPlanFilter) -> ApplicationTaskPlanFilter {
    match filter {
        TaskReadPlanFilter::PlanNeeded => ApplicationTaskPlanFilter::PlanNeeded,
        TaskReadPlanFilter::HasSteps => ApplicationTaskPlanFilter::HasSteps,
        TaskReadPlanFilter::IncompleteRequiredSteps => {
            ApplicationTaskPlanFilter::IncompleteRequiredSteps
        }
    }
}
pub(super) fn application_task_sort(sort: TaskReadSort) -> ApplicationTaskListSort {
    match sort {
        TaskReadSort::Seq => ApplicationTaskListSort::Seq,
        TaskReadSort::SeqDesc => ApplicationTaskListSort::SeqDesc,
        TaskReadSort::Title => ApplicationTaskListSort::Title,
        TaskReadSort::TitleDesc => ApplicationTaskListSort::TitleDesc,
        TaskReadSort::Status => ApplicationTaskListSort::Status,
        TaskReadSort::StatusDesc => ApplicationTaskListSort::StatusDesc,
        TaskReadSort::Position => ApplicationTaskListSort::Position,
        TaskReadSort::PositionDesc => ApplicationTaskListSort::PositionDesc,
        TaskReadSort::Priority => ApplicationTaskListSort::Priority,
        TaskReadSort::PriorityDesc => ApplicationTaskListSort::PriorityDesc,
        TaskReadSort::Assignee => ApplicationTaskListSort::Assignee,
        TaskReadSort::AssigneeDesc => ApplicationTaskListSort::AssigneeDesc,
        TaskReadSort::ScheduledAt => ApplicationTaskListSort::ScheduledAt,
        TaskReadSort::ScheduledAtDesc => ApplicationTaskListSort::ScheduledAtDesc,
        TaskReadSort::DueAt => ApplicationTaskListSort::DueAt,
        TaskReadSort::DueAtDesc => ApplicationTaskListSort::DueAtDesc,
        TaskReadSort::CreatedAt => ApplicationTaskListSort::CreatedAt,
        TaskReadSort::CreatedAtDesc => ApplicationTaskListSort::CreatedAtDesc,
        TaskReadSort::UpdatedAt => ApplicationTaskListSort::UpdatedAt,
        TaskReadSort::UpdatedAtDesc => ApplicationTaskListSort::UpdatedAtDesc,
    }
}

fn invalid(message: impl Into<String>) -> ApiError {
    KanbanError::InvalidInput(message.into()).into()
}
fn repeated<T: PartialEq>(values: &mut [T], name: &str, maximum: usize) -> Result<(), ApiError> {
    if values.len() > maximum {
        return Err(invalid(format!("{name} filter 超过 {maximum} 项")));
    }
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
    {
        return Err(invalid(format!("重复的 query parameter 值：{name}")));
    }
    Ok(())
}
fn bounded(value: &mut Option<String>, name: &str, maximum: usize) -> Result<(), ApiError> {
    if let Some(text) = value.take() {
        let text = text.trim();
        if text.chars().count() > maximum {
            return Err(invalid(format!("{name} 超过 {maximum} 个字符")));
        }
        if !text.is_empty() {
            *value = Some(text.to_owned());
        }
    }
    Ok(())
}
macro_rules! validate_query {
    ($name:ident, $ty:ty) => {
        pub(super) fn $name(query: &mut $ty) -> Result<(), ApiError> {
            repeated(&mut query.status, "status", MAX_TASK_READ_STATUSES)?;
            repeated(&mut query.priority, "priority", MAX_TASK_READ_PRIORITIES)?;
            repeated(&mut query.label, "label", MAX_TASK_READ_LABELS)?;
            repeated(
                &mut query.plan_filter,
                "plan_filter",
                MAX_TASK_READ_PLAN_FILTERS,
            )?;
            if query
                .label
                .iter()
                .any(|label| label.as_str().chars().count() > MAX_TASK_READ_LABEL_CHARS)
            {
                return Err(invalid("label 超过字符上限"));
            }
            bounded(
                &mut query.assignee,
                "assignee",
                MAX_TASK_READ_ASSIGNEE_CHARS,
            )?;
            bounded(&mut query.q, "q", MAX_TASK_READ_Q_CHARS)?;
            if query.limit > MAX_TASK_READ_LIMIT {
                return Err(invalid(format!("limit 必须小于等于 {MAX_TASK_READ_LIMIT}")));
            }
            if query.offset > i64::MAX as usize {
                return Err(invalid("offset 超过支持的范围"));
            }
            Ok(())
        }
    };
}
validate_query!(validate_list_query, ListTasksQuery);
validate_query!(validate_status_query, ListTasksByStatusQuery);
