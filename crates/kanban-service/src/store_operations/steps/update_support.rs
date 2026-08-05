use crate::{UpdateStepInput, error::StoreError, store_operations::shared::validate_task_id};

pub(crate) fn validate_update_step_input(
    task_id: &str,
    step_id: &str,
    input: &UpdateStepInput,
) -> Result<(), StoreError> {
    validate_task_id(task_id)?;
    if !step_id.trim().starts_with("step_") || step_id.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "step id must start with step_".to_owned(),
        ));
    }
    if input
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "step title is required when provided".to_owned(),
        ));
    }
    if input.position.is_some_and(|position| position < 0) {
        return Err(StoreError::InvalidInput(
            "step position must be non-negative".to_owned(),
        ));
    }
    if input.linked_task_id.is_some() && input.unlink_task {
        return Err(StoreError::InvalidInput(
            "linked_task_ref and unlink_task cannot be used together".to_owned(),
        ));
    }
    if input.title.is_none()
        && input.body.is_none()
        && input.linked_task_id.is_none()
        && !input.unlink_task
        && input.position.is_none()
        && input.required.is_none()
    {
        return Err(StoreError::InvalidInput(
            "step update requires at least one field".to_owned(),
        ));
    }
    if input.updated_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "updated_by is required".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.updated_at < 0 {
        return Err(StoreError::InvalidInput(
            "updated_at must be non-negative".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
