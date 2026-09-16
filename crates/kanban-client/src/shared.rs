use kanban_protocol::{CreateCommentRequest, CreateStepRequest, CreateTaskRequest};

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

#[cfg(test)]
mod tests {
    use super::*;

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
        let request = kanban_protocol::CreateCommentRequest {
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
            kanban_protocol::CreateCommentRequest {
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
}
