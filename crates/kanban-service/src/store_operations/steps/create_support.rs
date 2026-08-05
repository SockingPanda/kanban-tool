use crate::{
    CreateStepInput,
    domain::TaskStepRecord,
    error::StoreError,
    store_operations::shared::validate_task_id,
};

pub(crate) fn validate_create_step_input(
    task_id: &str,
    input: &CreateStepInput,
) -> Result<(), StoreError> {
    validate_task_id(task_id)?;
    if !input.id.trim().starts_with("step_") || input.id.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "step id must start with step_".to_owned(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "step title is required".to_owned(),
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
    if input.position.is_some_and(|position| position < 0) {
        return Err(StoreError::InvalidInput(
            "step position must be non-negative".to_owned(),
        ));
    }
    if input.created_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "created_by is required".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if !matches!(
        input.expected_plan_state.trim(),
        "unplanned" | "planned" | "not_required"
    ) {
        return Err(StoreError::InvalidInput(
            "expected_plan_state is invalid".to_owned(),
        ));
    }
    if !matches!(
        input.target_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review"
    ) {
        return Err(StoreError::InvalidInput(
            "target_status is invalid".to_owned(),
        ));
    }
    for (name, value) in [
        ("event_id", input.event_id.as_str()),
        ("plan_event_id", input.plan_event_id.as_str()),
        ("recompute_event_id", input.recompute_event_id.as_str()),
    ] {
        if !value.trim().starts_with("e_") || value.trim().len() <= 2 {
            return Err(StoreError::InvalidInput(format!(
                "{name} must start with e_"
            )));
        }
    }
    if input.created_at < 0 {
        return Err(StoreError::InvalidInput(
            "created_at must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
pub(crate) fn step_payload_matches(
    existing: &TaskStepRecord,
    title: &str,
    body: Option<&str>,
    linked_task_id: Option<&str>,
    position: i64,
    required: bool,
    created_by: &str,
) -> bool {
    existing.title == title
        && existing.body.as_deref() == body
        && existing.linked_task.as_ref().map(|task| task.id.as_str()) == linked_task_id
        && existing.position == position
        && existing.required == required
        && existing.created_by == created_by.trim()
}
