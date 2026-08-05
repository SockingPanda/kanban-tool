use crate::{CreateTaskInput, domain::TaskRecord, error::StoreError};

pub(crate) fn validate_create_task_input(input: &CreateTaskInput) -> Result<(), StoreError> {
    if !input.id.starts_with("t_") || input.id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(StoreError::InvalidInput("title is required".to_owned()));
    }
    if !matches!(input.status.as_str(), "triage" | "todo" | "scheduled") {
        return Err(StoreError::InvalidInput(
            "status must be triage, todo, or scheduled".to_owned(),
        ));
    }
    if !(0..=3).contains(&input.priority) {
        return Err(StoreError::InvalidInput(
            "priority must be between 0 and 3".to_owned(),
        ));
    }
    if input.max_retries.is_some_and(|value| value < 0) {
        return Err(StoreError::InvalidInput(
            "max_retries must be non-negative".to_owned(),
        ));
    }
    if input.created_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "created_by is required".to_owned(),
        ));
    }
    if input
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn canonical_payload_matches(
    existing: &TaskRecord,
    input: &CreateTaskInput,
    canonical_title: &str,
) -> bool {
    existing.status == input.status
        && existing.title == canonical_title
        && existing.description == input.description
        && existing.assignee == input.assignee
        && existing.priority == input.priority
        && existing.scheduled_at == input.scheduled_at
        && existing.due_at == input.due_at
        && existing.max_retries == input.max_retries
        && existing.metadata_json == input.metadata_json
        && existing.created_by == input.created_by
}
