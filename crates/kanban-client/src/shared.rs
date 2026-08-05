use kanban_contract::{CreateCommentRequest, CreateStepRequest, CreateTaskRequest, ListTasksQuery};

pub(crate) fn prepare_create_request(mut request: CreateTaskRequest) -> CreateTaskRequest {
    let task_id = request.task_id.get_or_insert_with(kanban_core::new_task_id);
    request
        .idempotency_key
        .get_or_insert_with(|| format!("task.create:{task_id}"));
    request
}

pub(crate) fn prepare_create_comment_request(
    mut request: CreateCommentRequest,
    _task_id: &str,
) -> CreateCommentRequest {
    request
        .idempotency_key
        .get_or_insert_with(|| format!("comment.create:{}", kanban_core::new_typed_id("c")));
    request
}

pub(crate) fn prepare_create_step_request(mut request: CreateStepRequest) -> CreateStepRequest {
    request
        .idempotency_key
        .get_or_insert_with(|| format!("step.create:{}", kanban_core::new_typed_id("step")));
    request
}

pub(crate) fn list_tasks_path(board: &str, query: &ListTasksQuery) -> String {
    let mut pairs = Vec::new();
    for status in &query.status {
        pairs.push(("status", status.as_str().to_owned()));
    }
    for priority in &query.priority {
        pairs.push(("priority", priority.get().to_string()));
    }
    for label in &query.label {
        pairs.push(("label", label.as_str().to_owned()));
    }
    for filter in &query.plan_filter {
        pairs.push(("plan_filter", filter.as_str().to_owned()));
    }
    if let Some(assignee) = query.assignee.as_deref() {
        pairs.push(("assignee", assignee.to_owned()));
    }
    if let Some(search) = query.q.as_deref() {
        pairs.push(("q", search.to_owned()));
    }
    pairs.push(("include_archived", query.include_archived.to_string()));
    pairs.push(("limit", query.limit.to_string()));
    pairs.push(("offset", query.offset.to_string()));
    pairs.push(("sort", query.sort.as_str().to_owned()));
    let query = pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={}", crate::transport::encode_path_segment(&value)))
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "/api/v1/boards/{}/tasks?{query}",
        crate::transport::encode_path_segment(board)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_contract::{ApiTaskPriority, ApiTaskStatus, TaskReadSort};

    #[test]
    fn create_request_gets_stable_entity_local_identifiers() {
        let request = prepare_create_request(CreateTaskRequest {
            task_id: None,
            idempotency_key: None,
            title: "Create".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 3,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata: None,
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: None,
        });
        let task_id = request.task_id.as_deref().unwrap();
        assert!(task_id.starts_with("t_"));
        assert_eq!(
            request.idempotency_key.as_deref(),
            Some(format!("task.create:{task_id}").as_str())
        );
    }

    #[test]
    fn comment_request_gets_unique_entity_local_idempotency_keys() {
        let request = kanban_contract::CreateCommentRequest {
            idempotency_key: None,
            author: None,
            body: " handoff ".into(),
            kind: None,
            author_type: None,
            agent_type: None,
            metadata: None,
        };
        let first = prepare_create_comment_request(request.clone(), "t_comment");
        let second = prepare_create_comment_request(request, "t_comment");
        let first_key = first.idempotency_key.as_deref().unwrap();
        let second_key = second.idempotency_key.as_deref().unwrap();
        assert!(first_key.starts_with("comment.create:c_"));
        assert!(second_key.starts_with("comment.create:c_"));
        assert_ne!(first_key, second_key);
    }

    #[test]
    fn comment_request_preserves_explicit_entity_local_idempotency_key() {
        let request = prepare_create_comment_request(
            kanban_contract::CreateCommentRequest {
                idempotency_key: Some("comment.retry:fixed".into()),
                author: None,
                body: "handoff".into(),
                kind: None,
                author_type: None,
                agent_type: None,
                metadata: None,
            },
            "t_comment",
        );
        assert_eq!(
            request.idempotency_key.as_deref(),
            Some("comment.retry:fixed")
        );
    }

    #[test]
    fn list_task_query_preserves_repeated_filters_and_escaping() {
        let query = ListTasksQuery {
            status: vec![ApiTaskStatus::Ready, ApiTaskStatus::Blocked],
            priority: vec![
                ApiTaskPriority::new(0).unwrap(),
                ApiTaskPriority::new(2).unwrap(),
            ],
            q: Some("a & b".into()),
            limit: 25,
            offset: 50,
            sort: TaskReadSort::UpdatedAtDesc,
            ..ListTasksQuery::default()
        };
        assert_eq!(
            list_tasks_path("team/one", &query),
            "/api/v1/boards/team%2Fone/tasks?status=ready&status=blocked&priority=0&priority=2&q=a%20%26%20b&include_archived=false&limit=25&offset=50&sort=-updated_at"
        );
    }
}
