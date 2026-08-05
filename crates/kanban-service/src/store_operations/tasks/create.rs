use turso::transaction::TransactionBehavior;

use crate::store_operations::dependencies::support::dependency_task_in_transaction;
use crate::store_operations::labels::{list_task_labels_in_transaction, resolve_label_in_transaction};
use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::create_support::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub status: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata_json: String,
    pub labels: Vec<String>,
    pub depends_on: Vec<String>,
    pub created_by: String,
}
impl TursoStore {
    pub async fn create_task(
        &self,
        board_selector: &str,
        input: CreateTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_create_task_input(&input)?;
        let board_selector = board_selector.trim();
        if board_selector.is_empty() {
            return Err(StoreError::InvalidInput("看板不能为空".to_owned()));
        }
        let title = input.title.trim().to_owned();
        let labels = canonical_refs(&input.labels);
        let depends_on = canonical_refs(&input.depends_on);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let board = first_row(
            transaction
                .query(
                    "SELECT id, slug, archived_at FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [board_selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        let board_slug = text_value(board.get_value(1)?, "boards.slug")?;
        if optional_integer_value(board.get_value(2)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "已归档看板不能创建任务".to_owned(),
            ));
        }

        if let Some(idempotency_key) = input.idempotency_key.as_deref() {
            let existing = first_row(
                transaction
                    .query(
                        &format!(
                            "{TASK_SELECT} WHERE t.board_id = ?1 AND t.idempotency_key = ?2 LIMIT 1"
                        ),
                        (board_id.as_str(), idempotency_key),
                    )
                    .await?,
            )
            .await;
            match existing {
                Ok(row) => {
                    let mut existing = task_from_row(row)?;
                    existing.labels =
                        list_task_labels_in_transaction(&transaction, &board_id, &existing.id)
                            .await?;
                    if canonical_payload_matches(&existing, &input, &title)
                        && task_relations_match(&transaction, &existing, &labels, &depends_on)
                            .await?
                    {
                        transaction.commit().await?;
                        return Ok(existing);
                    }
                    return Err(StoreError::IdempotencyConflict {
                        board_id,
                        key: idempotency_key.to_owned(),
                        existing_task_id: existing.id,
                    });
                }
                Err(turso::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StoreError::Turso(error)),
            }
        }

        let seq = first_row(
            transaction
                .query(
                    "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id = ?1",
                    [board_id.as_str()],
                )
                .await?,
        )
        .await?
        .get_value(0)
        .map_err(StoreError::from)
        .and_then(|value| integer_value(value, "tasks.seq"))?;
        let position = seq
            .checked_mul(1024)
            .ok_or_else(|| StoreError::InvalidInput("task sequence is too large".to_owned()))?;
        let now = now_ms();
        match transaction
                .execute(
                    "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, max_retries, metadata_json, lock_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?15, ?16, 0)",
                    (
                        input.id.as_str(),
                        board_id.as_str(),
                        seq,
                        input.idempotency_key.as_deref(),
                        title.as_str(),
                        input.description.as_deref(),
                        input.status.as_str(),
                        input.assignee.as_deref(),
                        input.priority,
                        position,
                        input.scheduled_at,
                        input.due_at,
                        input.created_by.as_str(),
                        now,
                        input.max_retries,
                        input.metadata_json.as_str(),
                    ),
                )
                .await
        {
            Ok(_) => {}
            Err(turso::Error::Constraint(message))
                if is_duplicate_task_id_constraint(&message) =>
            {
                return Err(StoreError::TaskConflict(input.id.clone()));
            }
            Err(error) => return Err(StoreError::Turso(error)),
        }
        transaction
                .execute(
                    "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) VALUES (?1, 'task', 'tasks', ?2, ?3, ?2, ?4, ?5, NULL, ?6, ?6, NULL)",
                    (
                        format!("kb://task/{}", input.id).as_str(),
                        input.id.as_str(),
                        board_id.as_str(),
                        title.as_str(),
                        input.description.as_deref(),
                        now,
                    ),
                )
                .await?;
        transaction
                .execute(
                    "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (?1, ?2, 'unplanned', NULL, ?3, ?4)",
                    (board_id.as_str(), input.id.as_str(), input.created_by.as_str(), now),
                )
                .await?;
        let event_id = format!("e_{}_created", input.id.trim_start_matches("t_"));
        let event_payload = format!(r#"{{"status":"{}"}}"#, input.status);
        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, NULL, 'task.created', ?4, ?5, ?6)",
                    (
                        event_id.as_str(),
                        board_id.as_str(),
                        input.id.as_str(),
                        input.created_by.as_str(),
                        event_payload.as_str(),
                        now,
                    ),
                )
                .await?;
        for label_ref in &labels {
            let label = resolve_label_in_transaction(&transaction, &board_id, label_ref)
                .await?
                .ok_or_else(|| StoreError::LabelNotFound(label_ref.to_owned()))?;
            let changed = transaction
                .execute(
                    "INSERT INTO task_labels(board_id, task_id, label_id, created_at) VALUES (:board_id, :task_id, :label_id, :created_at) ON CONFLICT(task_id, label_id) DO NOTHING",
                    (
                        (":board_id", board_id.as_str()),
                        (":task_id", input.id.as_str()),
                        (":label_id", label.id.as_str()),
                        (":created_at", now),
                    ),
                )
                .await?;
            if changed > 0 {
                let event_id = kanban_core::new_event_id();
                transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.label.added', :actor, json_object('label_id', :label_id, 'label', :label), :created_at)",
                        (
                            (":event_id", event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", input.id.as_str()),
                            (":actor", input.created_by.as_str()),
                            (":label_id", label.id.as_str()),
                            (":label", label.name.as_str()),
                            (":created_at", now),
                        ),
                    )
                    .await?;
            }
        }
        for parent_id in &depends_on {
            let parent = dependency_task_in_transaction(&transaction, parent_id).await?;
            if parent.board_id != board_id {
                return Err(StoreError::InvalidInput(
                    "cross-board dependency is not allowed".to_owned(),
                ));
            }
            if parent.id == input.id {
                return Err(StoreError::InvalidInput(
                    "dependency cannot point to itself".to_owned(),
                ));
            }
            transaction
                .execute(
                    "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (:board_id, :parent_task_id, :child_task_id, :created_at) ON CONFLICT(parent_task_id, child_task_id) DO NOTHING",
                    (
                        (":board_id", board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":child_task_id", input.id.as_str()),
                        (":created_at", now),
                    ),
                )
                .await?;
            let event_id = kanban_core::new_event_id();
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'dependency.added', :actor, json_object('parent_task_id', :parent_task_id), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", input.id.as_str()),
                        (":actor", input.created_by.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":created_at", now),
                    ),
                )
                .await?;
        }
        let mut task = task_from_row(
            first_row(
                transaction
                    .query(
                        &format!("{TASK_SELECT} WHERE t.board_id = ?1 AND t.id = ?2 LIMIT 1"),
                        (board_id.as_str(), input.id.as_str()),
                    )
                    .await?,
            )
            .await?,
        )?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, &input.id).await?;

        transaction.commit().await?;
        debug_assert_eq!(task.board_id, board_id);
        debug_assert_eq!(task.board_slug, board_slug);
        Ok(task)
    }
}

fn is_duplicate_task_id_constraint(message: &str) -> bool {
    message.starts_with("UNIQUE constraint failed: tasks.id")
        || message.starts_with("UNIQUE constraint failed: tasks.(id, board_id)")
        || message.starts_with("UNIQUE constraint failed: tasks.(board_id, id)")
}
