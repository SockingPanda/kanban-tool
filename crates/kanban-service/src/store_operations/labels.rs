use turso::transaction::TransactionBehavior;
use turso::{Connection, Row, transaction::Transaction};

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLabelInput {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsInput {
    pub names: Vec<String>,
    pub label_ids: Vec<String>,
    pub event_ids: Vec<String>,
    pub create_missing: bool,
    pub actor: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsRecord {
    pub task: TaskRecord,
    pub created_labels: Vec<LabelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTaskLabelInput {
    pub label_ref: String,
    pub event_id: String,
    pub actor: String,
    pub now: i64,
}

impl TursoStore {
    pub async fn list_board_labels(
        &self,
        board_selector: &str,
    ) -> Result<Vec<LabelRecord>, StoreError> {
        let connection = self.connection().await?;
        let board_id = active_board_id(&connection, board_selector).await?;
        list_labels_on_connection(&connection, &board_id).await
    }

    pub async fn create_board_label(
        &self,
        board_selector: &str,
        input: CreateLabelInput,
    ) -> Result<LabelRecord, StoreError> {
        validate_create_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_board_id_in_transaction(&transaction, board_selector).await?;
        let name = input.name.trim();
        if let Some(label) = label_by_name_in_transaction(&transaction, &board_id, name).await? {
            transaction.commit().await?;
            return Ok(label);
        }
        transaction
            .execute(
                "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (:id, :board_id, :name, :color, :created_at, :created_at)",
                (
                    (":id", input.id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":name", name),
                    (":color", input.color.as_deref()),
                    (":created_at", input.created_at),
                ),
            )
            .await?;
        let label = label_by_id_in_transaction(&transaction, &board_id, &input.id)
            .await?
            .ok_or_else(|| StoreError::LabelNotFound(input.id.clone()))?;
        transaction.commit().await?;
        Ok(label)
    }

    pub async fn list_task_labels(&self, task_id: &str) -> Result<Vec<LabelRecord>, StoreError> {
        validate_task_id(task_id)?;
        let connection = self.connection().await?;
        let board_id = active_task_board_id(&connection, task_id).await?;
        list_task_labels_on_connection(&connection, &board_id, task_id).await
    }

    pub async fn add_task_labels(
        &self,
        task_id: &str,
        input: AddTaskLabelsInput,
    ) -> Result<AddTaskLabelsRecord, StoreError> {
        validate_task_id(task_id)?;
        validate_add_task_labels_input(&input)?;
        if input.names.len() != input.label_ids.len() || input.names.len() != input.event_ids.len()
        {
            return Err(StoreError::InvalidInput(
                "label input vectors must have equal lengths".to_owned(),
            ));
        }
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_task_board_id_in_transaction(&transaction, task_id).await?;
        let mut created_labels = Vec::new();

        for ((name, label_id), event_id) in input
            .names
            .iter()
            .zip(input.label_ids.iter())
            .zip(input.event_ids.iter())
        {
            let label = match label_by_name_in_transaction(&transaction, &board_id, name).await? {
                Some(label) => label,
                None if input.create_missing => {
                    transaction
                        .execute(
                            "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (:id, :board_id, :name, NULL, :created_at, :created_at)",
                            (
                                (":id", label_id.as_str()),
                                (":board_id", board_id.as_str()),
                                (":name", name.as_str()),
                                (":created_at", input.now),
                            ),
                        )
                        .await?;
                    let label = label_by_id_in_transaction(&transaction, &board_id, label_id)
                        .await?
                        .ok_or_else(|| StoreError::LabelNotFound(label_id.clone()))?;
                    created_labels.push(label.clone());
                    label
                }
                None => return Err(StoreError::LabelNotFound(name.clone())),
            };
            let changed = transaction
                .execute(
                    "INSERT INTO task_labels(board_id, task_id, label_id, created_at) VALUES (:board_id, :task_id, :label_id, :created_at) ON CONFLICT(task_id, label_id) DO NOTHING",
                    (
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":label_id", label.id.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
            if changed > 0 {
                transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.label.added', :actor, json_object('label_id', :label_id, 'label', :label), :created_at)",
                        (
                            (":event_id", event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":actor", input.actor.as_str()),
                            (":label_id", label.id.as_str()),
                            (":label", label.name.as_str()),
                            (":created_at", input.now),
                        ),
                    )
                    .await?;
            }
        }

        let mut task = task_in_transaction(&transaction, task_id).await?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, task_id).await?;
        transaction.commit().await?;
        Ok(AddTaskLabelsRecord {
            task,
            created_labels,
        })
    }

    pub async fn remove_task_label(
        &self,
        task_id: &str,
        input: RemoveTaskLabelInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_task_id(task_id)?;
        validate_remove_task_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_task_board_id_in_transaction(&transaction, task_id).await?;
        let label = resolve_label_in_transaction(&transaction, &board_id, &input.label_ref)
            .await?
            .ok_or_else(|| StoreError::LabelNotFound(input.label_ref.trim().to_owned()))?;
        let changed = transaction
            .execute(
                "DELETE FROM task_labels WHERE board_id = :board_id AND task_id = :task_id AND label_id = :label_id",
                (
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":label_id", label.id.as_str()),
                ),
            )
            .await?;
        if changed > 0 {
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.label.removed', :actor, json_object('label_id', :label_id, 'label', :label), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", input.actor.as_str()),
                        (":label_id", label.id.as_str()),
                        (":label", label.name.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        }
        let mut task = task_in_transaction(&transaction, task_id).await?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, task_id).await?;
        transaction.commit().await?;
        Ok(task)
    }
}

fn validate_task_id(task_id: &str) -> Result<(), StoreError> {
    if !task_id.trim().starts_with("t_") || task_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_label_input(input: &CreateLabelInput) -> Result<(), StoreError> {
    if !input.id.trim().starts_with("l_") || input.id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "label id must start with l_".to_owned(),
        ));
    }
    if input.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "label name is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_add_task_labels_input(input: &AddTaskLabelsInput) -> Result<(), StoreError> {
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.names.is_empty() {
        return Err(StoreError::InvalidInput(
            "at least one label name is required".to_owned(),
        ));
    }
    if input.names.iter().any(|name| name.trim().is_empty()) {
        return Err(StoreError::InvalidInput(
            "label name is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_remove_task_label_input(input: &RemoveTaskLabelInput) -> Result<(), StoreError> {
    if input.label_ref.trim().is_empty() {
        return Err(StoreError::InvalidInput("label id is required".to_owned()));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    Ok(())
}

async fn active_board_id(
    connection: &Connection,
    board_selector: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT id FROM boards WHERE (id = :selector OR slug = :selector) AND archived_at IS NULL LIMIT 1",
                [(":selector", board_selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board_selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn active_board_id_in_transaction(
    transaction: &Transaction<'_>,
    board_selector: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id FROM boards WHERE (id = :selector OR slug = :selector) AND archived_at IS NULL LIMIT 1",
                [(":selector", board_selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board_selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn active_task_board_id(
    connection: &Connection,
    task_id: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT t.board_id, t.status, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                [(":task_id", task_id.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    active_task_board_from_row(row, task_id)
}

async fn active_task_board_id_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT t.board_id, t.status, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                [(":task_id", task_id.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    active_task_board_from_row(row, task_id)
}

fn active_task_board_from_row(row: Row, task_id: &str) -> Result<String, StoreError> {
    let board_id = text_value(row.get_value(0)?, "tasks.board_id")?;
    let status = text_value(row.get_value(1)?, "tasks.status")?;
    let task_archived = optional_integer_value(row.get_value(2)?, "tasks.archived_at")?;
    let board_archived = optional_integer_value(row.get_value(3)?, "boards.archived_at")?;
    if status == "archived" || task_archived.is_some() || board_archived.is_some() {
        return Err(StoreError::TaskNotFound(task_id.to_owned()));
    }
    Ok(board_id)
}

async fn list_labels_on_connection(
    connection: &Connection,
    board_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id ORDER BY name ASC, id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn list_task_labels_on_connection(
    connection: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = :board_id AND tl.task_id = :task_id ORDER BY l.name ASC, l.id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn labels_from_rows(rows: &mut turso::Rows) -> Result<Vec<LabelRecord>, StoreError> {
    let mut labels = Vec::new();
    while let Some(row) = rows.next().await? {
        labels.push(label_from_row(row)?);
    }
    Ok(labels)
}

pub(crate) async fn list_task_labels_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    task_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = transaction
        .query(
            "SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = :board_id AND tl.task_id = :task_id ORDER BY l.name ASC, l.id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn task_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
        transaction
            .query(
                &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                [(":task_id", task_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    task_from_row(row)
}

async fn label_by_name_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    name: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id AND name = :name LIMIT 1",
                [(":board_id", board_id), (":name", name)],
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => Ok(Some(label_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

async fn label_by_id_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    label_id: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id AND id = :label_id LIMIT 1",
                [(":board_id", board_id), (":label_id", label_id)],
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => Ok(Some(label_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

pub(crate) async fn resolve_label_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    label_ref: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let label_ref = label_ref.trim();
    if let Some(label) = label_by_name_in_transaction(transaction, board_id, label_ref).await? {
        return Ok(Some(label));
    }
    if label_ref.starts_with("l_") {
        return label_by_id_in_transaction(transaction, board_id, label_ref).await;
    }
    Ok(None)
}

fn label_from_row(row: Row) -> Result<LabelRecord, StoreError> {
    Ok(LabelRecord {
        id: text_value(row.get_value(0)?, "labels.id")?,
        board_id: text_value(row.get_value(1)?, "labels.board_id")?,
        name: text_value(row.get_value(2)?, "labels.name")?,
        color: optional_text_value(row.get_value(3)?, "labels.color")?,
        created_at: integer_value(row.get_value(4)?, "labels.created_at")?,
        updated_at: integer_value(row.get_value(5)?, "labels.updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{create_input, store};

    fn create_label_input(id: &str, name: &str) -> CreateLabelInput {
        CreateLabelInput {
            id: id.to_owned(),
            name: name.to_owned(),
            color: Some("#4477aa".to_owned()),
            created_at: 100,
        }
    }

    fn add_input(
        name: &str,
        label_id: &str,
        event_id: &str,
        create_missing: bool,
    ) -> AddTaskLabelsInput {
        AddTaskLabelsInput {
            names: vec![name.to_owned()],
            label_ids: vec![label_id.to_owned()],
            event_ids: vec![event_id.to_owned()],
            create_missing,
            actor: "label-test".to_owned(),
            now: 100,
        }
    }

    #[tokio::test]
    async fn board_labels_trim_names_and_keep_duplicate_create_idempotent() {
        let (_directory, store, _path) = store("labels-create").await;
        store.initialize().await.expect("initialize");

        let first = store
            .create_board_label("default", create_label_input("l_first", "  urgent  "))
            .await
            .expect("create label");
        assert_eq!(first.name, "urgent");
        assert_eq!(first.color.as_deref(), Some("#4477aa"));

        let duplicate = store
            .create_board_label("default", create_label_input("l_duplicate", "urgent"))
            .await
            .expect("duplicate create returns existing label");
        assert_eq!(duplicate.id, first.id);

        let labels = store
            .list_board_labels("default")
            .await
            .expect("list labels");
        assert_eq!(labels, vec![first]);
    }

    #[tokio::test]
    async fn task_label_attach_is_idempotent_and_emits_one_event() {
        let (_directory, store, _path) = store("labels-attach").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_labels_attach", Some("labels-1"), "Labels"),
            )
            .await
            .expect("create task");
        store
            .create_board_label("default", create_label_input("l_attach", "urgent"))
            .await
            .expect("create label");

        let added = store
            .add_task_labels(
                "t_labels_attach",
                add_input("urgent", "l_generated", "evt-label-add-1", false),
            )
            .await
            .expect("attach label");
        assert_eq!(added.task.labels.len(), 1);
        assert!(added.created_labels.is_empty());

        let duplicate = store
            .add_task_labels(
                "t_labels_attach",
                add_input("urgent", "l_generated-duplicate", "evt-label-add-2", false),
            )
            .await
            .expect("duplicate attach");
        assert_eq!(duplicate.task.labels, added.task.labels);

        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = 't_labels_attach' AND kind = 'task.label.added'",
                    (),
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(row.get_value(0).unwrap(), "event count").unwrap(),
            1
        );

        let removed = store
            .remove_task_label(
                "t_labels_attach",
                RemoveTaskLabelInput {
                    label_ref: "l_attach".to_owned(),
                    event_id: "evt-label-remove-1".to_owned(),
                    actor: "label-test".to_owned(),
                    now: 101,
                },
            )
            .await
            .expect("remove label");
        assert!(removed.labels.is_empty());
    }

    #[tokio::test]
    async fn task_labels_are_board_isolated_and_active_guarded() {
        let (_directory, store, _path) = store("labels-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        store
            .create_board_label("other", create_label_input("l_other", "shared"))
            .await
            .expect("create label on other board");
        store
            .create_task(
                "default",
                create_input("t_labels_isolation", Some("labels-2"), "Labels"),
            )
            .await
            .expect("create task");

        let missing = store
            .add_task_labels(
                "t_labels_isolation",
                add_input("shared", "l_default", "evt-label-missing", false),
            )
            .await
            .expect_err("other-board label must not cross the board boundary");
        assert!(matches!(missing, StoreError::LabelNotFound(name) if name == "shared"));

        let created = store
            .add_task_labels(
                "t_labels_isolation",
                add_input("shared", "l_default", "evt-label-created", true),
            )
            .await
            .expect("create default-board label");
        assert_eq!(created.created_labels.len(), 1);
        assert_eq!(created.task.labels.len(), 1);
        assert_eq!(created.task.labels[0].board_id, "b_default");

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 200 WHERE id = 't_labels_isolation'",
                (),
            )
            .await
            .expect("archive task");
        let archived = store
            .list_task_labels("t_labels_isolation")
            .await
            .expect_err("archived task must be guarded");
        assert!(
            matches!(archived, StoreError::TaskNotFound(task_id) if task_id == "t_labels_isolation")
        );
    }
}
