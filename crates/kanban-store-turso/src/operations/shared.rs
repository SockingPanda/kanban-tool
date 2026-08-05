use crate::error::StoreError;

pub(crate) fn validate_task_id(task_id: &str) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn canonical_ready_status(
    title: &str,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    dependencies_done: bool,
    now: i64,
) -> &'static str {
    if title.trim().is_empty() || description.is_none_or(|value| value.trim().is_empty()) {
        return "triage";
    }
    if scheduled_at.is_some_and(|scheduled| scheduled > now) {
        return "scheduled";
    }
    if !dependencies_done {
        return "todo";
    }
    "ready"
}
