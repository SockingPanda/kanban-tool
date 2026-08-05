use crate::{
    CreateTaskInput,
    domain::TaskRecord,
    error::StoreError,
    shared::{first_row, text_value},
};

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

pub(crate) fn canonical_refs(values: &[String]) -> Vec<String> {
    let mut refs = values
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

pub(crate) async fn task_relations_match(
    transaction: &turso::transaction::Transaction<'_>,
    task: &TaskRecord,
    labels: &[String],
    depends_on: &[String],
) -> Result<bool, StoreError> {
    let mut rows = transaction
        .query(
            "SELECT l.id, l.name FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = :board_id AND tl.task_id = :task_id ORDER BY l.id",
            [(":board_id", task.board_id.as_str()), (":task_id", task.id.as_str())],
        )
        .await?;
    let mut current_labels = Vec::new();
    while let Some(row) = rows.next().await? {
        current_labels.push(text_value(row.get_value(0)?, "labels.id")?);
    }
    current_labels.sort();
    let mut expected_labels = Vec::new();
    for label_ref in labels {
        let row = first_row(
            transaction
                .query(
                    "SELECT id FROM labels WHERE board_id = :board_id AND (id = :label_ref OR name = :label_ref) LIMIT 1",
                    [(":board_id", task.board_id.as_str()), (":label_ref", label_ref.as_str())],
                )
                .await?,
        )
        .await;
        let Ok(row) = row else {
            return Ok(false);
        };
        expected_labels.push(text_value(row.get_value(0)?, "labels.id")?);
    }
    expected_labels.sort();
    expected_labels.dedup();
    if current_labels != expected_labels {
        return Ok(false);
    }
    let mut rows = transaction
        .query(
            "SELECT parent_task_id FROM task_dependencies WHERE board_id = :board_id AND child_task_id = :task_id ORDER BY parent_task_id",
            [(":board_id", task.board_id.as_str()), (":task_id", task.id.as_str())],
        )
        .await?;
    let mut current_parents = Vec::new();
    while let Some(row) = rows.next().await? {
        current_parents.push(text_value(
            row.get_value(0)?,
            "task_dependencies.parent_task_id",
        )?);
    }
    current_parents.sort();
    let mut expected_parents = depends_on.to_vec();
    expected_parents.sort();
    expected_parents.dedup();
    Ok(current_parents == expected_parents)
}
