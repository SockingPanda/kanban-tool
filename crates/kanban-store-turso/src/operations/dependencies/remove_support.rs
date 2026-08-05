use crate::{RemoveDependencyInput, error::StoreError, operations::shared::validate_task_id};

pub(crate) fn validate_remove_dependency_input(
    child_task_id: &str,
    parent_task_id: &str,
    input: &RemoveDependencyInput,
) -> Result<(), StoreError> {
    validate_task_id(child_task_id)?;
    validate_task_id(parent_task_id)?;
    if child_task_id.trim() == parent_task_id.trim() {
        return Err(StoreError::InvalidInput(
            "dependency cannot point to itself".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
