use crate::error::ApiError;
use kanban_protocol::{
    ApiTaskPriority, ApiTaskStatus, ListTasksQuery, MAX_TASK_READ_ASSIGNEE_CHARS,
    MAX_TASK_READ_LABEL_CHARS, MAX_TASK_READ_LABELS, MAX_TASK_READ_LIMIT,
    MAX_TASK_READ_PLAN_FILTERS, MAX_TASK_READ_PRIORITIES, MAX_TASK_READ_Q_CHARS,
    MAX_TASK_READ_QUERY_BYTES, MAX_TASK_READ_QUERY_PAIRS, MAX_TASK_READ_STATUSES, TaskReadLabel,
    TaskReadPlanFilter, TaskReadSort,
};
use kanban_service::{KanbanError, TaskStatus};
use kanban_service::{
    TaskListSort as ApplicationTaskListSort, TaskPlanFilter as ApplicationTaskPlanFilter,
};
use std::{collections::BTreeSet, str::FromStr};

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

pub(super) fn parse_list_tasks_query(raw_query: Option<&str>) -> Result<ListTasksQuery, ApiError> {
    let mut query = ListTasksQuery::default();
    let mut scalar_parameters = BTreeSet::new();
    let Some(raw_query) = raw_query else {
        return Ok(query);
    };
    if raw_query.is_empty() {
        return Ok(query);
    }
    if raw_query.len() > MAX_TASK_READ_QUERY_BYTES {
        return Err(KanbanError::InvalidInput(format!(
            "task-read raw query 超过 {MAX_TASK_READ_QUERY_BYTES} 字节"
        ))
        .into());
    }
    let pairs = raw_query.split('&').collect::<Vec<_>>();
    if pairs.len() > MAX_TASK_READ_QUERY_PAIRS {
        return Err(KanbanError::InvalidInput(format!(
            "task-read query 超过 {MAX_TASK_READ_QUERY_PAIRS} 个参数对"
        ))
        .into());
    }
    for pair in pairs {
        let (encoded_key, encoded_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode_query_component(encoded_key)?;
        let value = decode_query_component(encoded_value)?;
        match key.as_str() {
            "status" => {
                let status = ApiTaskStatus::from_str(value.trim()).map_err(|()| {
                    KanbanError::InvalidInput(format!("未知的 status filter：{value}"))
                })?;
                push_repeated(&mut query.status, status, "status", MAX_TASK_READ_STATUSES)?;
            }
            "priority" => {
                let priority = value
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .and_then(|value| ApiTaskPriority::try_from(value).ok())
                    .ok_or_else(|| {
                        KanbanError::InvalidInput("priority 必须在 0 到 3 之间".to_owned())
                    })?;
                push_repeated(
                    &mut query.priority,
                    priority,
                    "priority",
                    MAX_TASK_READ_PRIORITIES,
                )?;
            }
            "label" => {
                if value.chars().count() > MAX_TASK_READ_LABEL_CHARS {
                    return Err(KanbanError::InvalidInput(format!(
                        "label 超过 {MAX_TASK_READ_LABEL_CHARS} 个字符"
                    ))
                    .into());
                }
                let label = TaskReadLabel::new(value).ok_or_else(|| {
                    KanbanError::InvalidInput("label 必须包含非空白字符".to_owned())
                })?;
                push_repeated(&mut query.label, label, "label", MAX_TASK_READ_LABELS)?;
            }
            "plan_filter" => {
                let filter = TaskReadPlanFilter::from_str(value.trim()).map_err(|()| {
                    KanbanError::InvalidInput(format!("未知的 plan_filter：{value}"))
                })?;
                push_repeated(
                    &mut query.plan_filter,
                    filter,
                    "plan_filter",
                    MAX_TASK_READ_PLAN_FILTERS,
                )?;
            }
            "assignee" => {
                scalar(&mut scalar_parameters, "assignee")?;
                query.assignee = bounded_optional(value, "assignee", MAX_TASK_READ_ASSIGNEE_CHARS)?;
            }
            "q" => {
                scalar(&mut scalar_parameters, "q")?;
                query.q = bounded_optional(value, "q", MAX_TASK_READ_Q_CHARS)?;
            }
            "include_archived" => {
                scalar(&mut scalar_parameters, "include_archived")?;
                query.include_archived = value.parse::<bool>().map_err(|_| {
                    KanbanError::InvalidInput(format!("include_archived 无效：{value}"))
                })?;
            }
            "limit" => {
                scalar(&mut scalar_parameters, "limit")?;
                query.limit = value
                    .parse::<usize>()
                    .map_err(|_| KanbanError::InvalidInput(format!("limit 无效：{value}")))?;
                if query.limit > MAX_TASK_READ_LIMIT {
                    return Err(KanbanError::InvalidInput(format!(
                        "limit 必须小于等于 {MAX_TASK_READ_LIMIT}"
                    ))
                    .into());
                }
            }
            "offset" => {
                scalar(&mut scalar_parameters, "offset")?;
                query.offset = value
                    .parse::<usize>()
                    .map_err(|_| KanbanError::InvalidInput(format!("offset 无效：{value}")))?;
                if query.offset > i64::MAX as usize {
                    return Err(
                        KanbanError::InvalidInput("offset 超过支持的范围".to_owned()).into(),
                    );
                }
            }
            "sort" => {
                scalar(&mut scalar_parameters, "sort")?;
                query.sort = TaskReadSort::from_str(value.trim())
                    .map_err(|()| KanbanError::InvalidInput(format!("不支持的 sort：{value}")))?;
            }
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "未知的 task-read query parameter：{key}"
                ))
                .into());
            }
        }
    }
    Ok(query)
}

pub(super) fn decode_query_component(encoded: &str) -> Result<String, ApiError> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                let high = bytes
                    .get(index + 1)
                    .and_then(|byte| hex_value(*byte))
                    .ok_or_else(|| {
                        KanbanError::InvalidInput(
                            "query 中包含格式错误的 percent-encoding".to_owned(),
                        )
                    })?;
                let low = bytes
                    .get(index + 2)
                    .and_then(|byte| hex_value(*byte))
                    .ok_or_else(|| {
                        KanbanError::InvalidInput(
                            "query 中包含格式错误的 percent-encoding".to_owned(),
                        )
                    })?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded)
        .map_err(|_| KanbanError::InvalidInput("query 不是有效的 UTF-8".to_owned()).into())
}

pub(super) const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(super) fn scalar(
    seen: &mut BTreeSet<&'static str>,
    name: &'static str,
) -> Result<(), ApiError> {
    if seen.insert(name) {
        Ok(())
    } else {
        Err(KanbanError::InvalidInput(format!("重复的 scalar query parameter：{name}")).into())
    }
}

fn push_repeated<T: PartialEq>(
    values: &mut Vec<T>,
    value: T,
    name: &'static str,
    maximum: usize,
) -> Result<(), ApiError> {
    if values.len() >= maximum {
        return Err(KanbanError::InvalidInput(format!(
            "{name} query parameter 过多：最多 {maximum} 个"
        ))
        .into());
    }
    if values.contains(&value) {
        return Err(KanbanError::InvalidInput(format!("重复的 query parameter 值：{name}")).into());
    }
    values.push(value);
    Ok(())
}

pub(super) fn bounded_optional(
    value: String,
    name: &'static str,
    maximum_chars: usize,
) -> Result<Option<String>, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum_chars {
        return Err(
            KanbanError::InvalidInput(format!("{name} 超过 {maximum_chars} 个字符")).into(),
        );
    }
    Ok(Some(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_list_query_parses_repeated_filters_and_encoded_search() {
        let query = parse_list_tasks_query(Some(
            "status=ready&status=blocked&priority=0&priority=2&q=a%20%26%20b&limit=25&offset=50&sort=-updated_at",
        ))
        .unwrap();
        assert_eq!(
            query.status,
            vec![ApiTaskStatus::Ready, ApiTaskStatus::Blocked]
        );
        assert_eq!(
            query
                .priority
                .into_iter()
                .map(ApiTaskPriority::get)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(query.q.as_deref(), Some("a & b"));
        assert_eq!(query.limit, 25);
        assert_eq!(query.offset, 50);
        assert_eq!(query.sort, TaskReadSort::UpdatedAtDesc);
    }

    #[test]
    fn task_list_query_rejects_duplicate_and_unknown_parameters() {
        assert!(
            parse_list_tasks_query(Some("limit=10&limit=20"))
                .unwrap_err()
                .0
                .to_string()
                .contains("重复")
        );
        assert!(parse_list_tasks_query(Some("future=true")).is_err());
        assert!(parse_list_tasks_query(Some("q=%ZZ")).is_err());
    }
}
