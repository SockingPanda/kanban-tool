use crate::{AddDependencyInput, error::StoreError, store_operations::shared::validate_task_id};

pub(crate) fn validate_add_dependency_input(
    child_task_id: &str,
    parent_task_id: &str,
    input: &AddDependencyInput,
) -> Result<(), StoreError> {
    validate_task_id(child_task_id)?;
    validate_task_id(parent_task_id)?;
    if child_task_id.trim() == parent_task_id.trim() {
        return Err(StoreError::InvalidInput(
            "dependency cannot point to itself".to_owned(),
        ));
    }
    if input.expected_child_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_child_lock_version must be non-negative".to_owned(),
        ));
    }
    if !matches!(
        input.target_child_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done"
    ) {
        return Err(StoreError::InvalidInput(
            "target_child_status is invalid".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    for (name, value) in [
        ("event_id", input.event_id.as_str()),
        ("recompute_event_id", input.recompute_event_id.as_str()),
    ] {
        if !value.trim().starts_with("e_") || value.trim().len() <= 2 {
            return Err(StoreError::InvalidInput(format!(
                "{name} must start with e_"
            )));
        }
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
