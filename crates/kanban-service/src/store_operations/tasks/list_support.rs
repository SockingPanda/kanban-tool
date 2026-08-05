use crate::{
    error::StoreError,
    shared::Value,
    store_operations::tasks::{TaskListOptions, TaskListSort, TaskPlanFilter},
};

pub(crate) const TASK_FROM: &str = "FROM tasks AS t JOIN boards AS b ON b.id = t.board_id";

pub(crate) fn validate_task_list_options(options: &TaskListOptions) -> Result<(), StoreError> {
    if options.limit > 1000 {
        return Err(StoreError::InvalidInput("limit must be <= 1000".to_owned()));
    }
    if i64::try_from(options.offset).is_err() {
        return Err(StoreError::InvalidInput("offset is too large".to_owned()));
    }
    for status in &options.statuses {
        if !matches!(
            status.as_str(),
            "triage"
                | "todo"
                | "scheduled"
                | "ready"
                | "running"
                | "blocked"
                | "review"
                | "done"
                | "archived"
        ) {
            return Err(StoreError::InvalidInput(format!(
                "unknown task status: {status}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn task_list_where(
    board_id: &str,
    board_slug: &str,
    options: &TaskListOptions,
) -> (String, Vec<(String, Value)>) {
    let mut clauses = vec!["t.board_id = :board_id".to_owned()];
    let mut params = vec![(":board_id".to_owned(), Value::Text(board_id.to_owned()))];

    if !options.include_archived {
        clauses.push("t.status != 'archived'".to_owned());
    }
    if !options.statuses.is_empty() {
        let names = options
            .statuses
            .iter()
            .enumerate()
            .map(|(index, _)| format!(":status_{index}"))
            .collect::<Vec<_>>();
        clauses.push(format!("t.status IN ({})", names.join(", ")));
        params.extend(
            options.statuses.iter().enumerate().map(|(index, status)| {
                (
                    format!(":status_{index}"),
                    Value::Text(status.to_string()),
                )
            }),
        );
    }
    if !options.priorities.is_empty() {
        let names = options
            .priorities
            .iter()
            .enumerate()
            .map(|(index, _)| format!(":priority_{index}"))
            .collect::<Vec<_>>();
        clauses.push(format!("t.priority IN ({})", names.join(", ")));
        params.extend(
            options
                .priorities
                .iter()
                .enumerate()
                .map(|(index, priority)| (format!(":priority_{index}"), Value::Integer(*priority))),
        );
    }

    for (index, label) in options.labels.iter().enumerate() {
        let name = format!(":label_{index}");
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = t.board_id AND tl.task_id = t.id AND (l.name = {name} OR l.id = {name}))"
        ));
        params.push((name, Value::Text(label.trim().to_owned())));
    }

    for filter in &options.plan_filters {
        let clause = match filter {
            TaskPlanFilter::PlanNeeded => {
                "t.status NOT IN ('done', 'archived') AND NOT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id) AND NOT EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = t.board_id AND ep.task_id = t.id AND ep.state = 'not_required')"
            }
            TaskPlanFilter::HasSteps => {
                "EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id)"
            }
            TaskPlanFilter::IncompleteRequiredSteps => {
                "EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1 AND s.status NOT IN ('done', 'skipped'))"
            }
        };
        clauses.push(format!("({clause})"));
    }
    if let Some(assignee) = options
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("t.assignee = :assignee".to_owned());
        params.push((":assignee".to_owned(), Value::Text(assignee.to_owned())));
    }
    if let Some(q) = options
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        append_task_search_filter(&mut clauses, &mut params, board_id, board_slug, q);
    }

    (format!("WHERE {}", clauses.join(" AND ")), params)
}

pub(crate) fn append_task_search_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<(String, Value)>,
    board_id: &str,
    board_slug: &str,
    query: &str,
) {
    if query.starts_with("t_") {
        clauses.push("t.id = :q_task_id".to_owned());
        params.push((":q_task_id".to_owned(), Value::Text(query.to_owned())));
        return;
    }

    if query.starts_with('#') {
        let Some(seq) = parse_task_seq(query) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    if let Some((board_ref, seq_ref)) = query.split_once('#') {
        if board_ref.is_empty() || seq_ref.is_empty() {
            clauses.push("0 = 1".to_owned());
            return;
        }
        let Some(seq) = parse_task_seq(seq_ref) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        if board_ref != board_id && board_ref != board_slug {
            clauses.push("0 = 1".to_owned());
            return;
        }
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    if query.chars().all(|character| character.is_ascii_digit()) {
        let Some(seq) = parse_task_seq(query) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    let needle = format!("%{}%", sqlite_like_literal(&query.to_lowercase()));
    clauses.push(
        "(lower(t.title) LIKE :q_text ESCAPE '\\' OR lower(COALESCE(t.description, '')) LIKE :q_text ESCAPE '\\')"
            .to_owned(),
    );
    params.push((":q_text".to_owned(), Value::Text(needle)));
}

pub(crate) fn parse_task_seq(value: &str) -> Option<i64> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

pub(crate) fn sqlite_like_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

pub(crate) fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Seq => "t.seq ASC, t.id ASC",
        TaskListSort::SeqDesc => "t.seq DESC, t.id DESC",
        TaskListSort::Title => "lower(t.title) ASC, t.seq ASC, t.id ASC",
        TaskListSort::TitleDesc => "lower(t.title) DESC, t.seq DESC, t.id DESC",
        TaskListSort::Status => {
            "CASE t.status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END ASC, t.position ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::StatusDesc => {
            "CASE t.status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END DESC, t.position DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::Position => "t.position ASC, t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::PositionDesc => "t.position DESC, t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::Priority => "t.priority ASC, t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::PriorityDesc => "t.priority DESC, t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::Assignee => {
            "COALESCE(t.assignee, t.claim_owner, '') ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::AssigneeDesc => {
            "COALESCE(t.assignee, t.claim_owner, '') DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::ScheduledAt => {
            "COALESCE(t.scheduled_at, 9223372036854775807) ASC, t.created_at ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::ScheduledAtDesc => {
            "COALESCE(t.scheduled_at, -9223372036854775808) DESC, t.created_at DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::CreatedAt => "t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::CreatedAtDesc => "t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::UpdatedAt => "t.updated_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::UpdatedAtDesc => "t.updated_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::DueAt => {
            "COALESCE(t.due_at, 9223372036854775807) ASC, t.created_at ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::DueAtDesc => {
            "COALESCE(t.due_at, -9223372036854775808) DESC, t.created_at DESC, t.seq DESC, t.id DESC"
        }
    }
}
