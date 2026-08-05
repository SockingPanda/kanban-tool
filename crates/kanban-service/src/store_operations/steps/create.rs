use turso::transaction::TransactionBehavior;

use crate::store_operations::shared::canonical_ready_status;
use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::create_support::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStepInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub created_by: String,
    pub event_id: String,
    pub plan_event_id: String,
    pub recompute_event_id: String,
    pub created_at: i64,
    pub expected_lock_version: i64,
    pub expected_plan_state: String,
    pub target_status: String,
}
impl TursoStore {
    /// Create one execution-plan step and apply the associated plan/status
    /// changes in a single immediate transaction. The application service
    /// supplies the expected parent facts; this method re-reads them and
    /// refuses stale writes before touching any canonical row.
    pub async fn create_step(
        &self,
        task_id: &str,
        input: CreateStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_create_step_input(task_id, &input)?;
        let title = input.title.trim().to_owned();
        let body = input.body.map(|body| body.trim().to_owned());
        let idempotency_key = input
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let parent_row = first_row(
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
        let parent = task_from_row(parent_row)?;
        let board_id = parent.board_id.clone();
        if parent.archived_at.is_some() || parent.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived parent task cannot receive steps".to_owned(),
            ));
        }
        let board_row = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board_row.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive steps".to_owned(),
            ));
        }
        if let Some(idempotency_key) = idempotency_key.as_deref() {
            let existing = first_row(
                    transaction
                        .query(
                            "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND idempotency_key = :idempotency_key LIMIT 1",
                            [
                                (":board_id", board_id.as_str()),
                                (":parent_task_id", parent.id.as_str()),
                                (":idempotency_key", idempotency_key),
                            ],
                        )
                        .await?,
                )
                .await;
            match existing {
                Ok(row) => {
                    let existing = step_from_row(&transaction, row).await?;
                    let effective_position = input.position.unwrap_or(existing.position);
                    if step_payload_matches(
                        &existing,
                        &title,
                        body.as_deref(),
                        input.linked_task_id.as_deref(),
                        effective_position,
                        input.required,
                        &input.created_by,
                    ) {
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

        if input.expected_lock_version != parent.lock_version {
            return Err(StoreError::InvalidTransition(
                "step create requires matching fresh parent task".to_owned(),
            ));
        }
        if input.expected_plan_state.trim() != parent.execution_plan_state {
            return Err(StoreError::InvalidTransition(
                "step create requires matching execution plan".to_owned(),
            ));
        }
        if !matches!(
            parent.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review"
        ) {
            return Err(StoreError::InvalidTransition(format!(
                "cannot create a step for {} task",
                parent.status
            )));
        }

        if let Some(linked_task_id) = input.linked_task_id.as_deref() {
            let linked_row = first_row(
                transaction
                    .query(
                        &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                        [(":task_id", linked_task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => {
                    StoreError::TaskNotFound(linked_task_id.to_owned())
                }
                other => StoreError::Turso(other),
            })?;
            let linked_task = task_from_row(linked_row)?;
            if linked_task.board_id != board_id {
                return Err(StoreError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked_task.id == parent.id {
                return Err(StoreError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked_task.archived_at.is_some() || linked_task.status == "archived" {
                return Err(StoreError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }

        let position = match input.position {
            Some(position) => position,
            None => {
                let row = first_row(
                        transaction
                            .query(
                                "SELECT COALESCE(MAX(position), 0) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id",
                                [
                                    (":board_id", board_id.as_str()),
                                    (":parent_task_id", parent.id.as_str()),
                                ],
                            )
                            .await?,
                    )
                    .await?;
                integer_value(row.get_value(0)?, "task_steps.max_position")?
                    .checked_add(1024)
                    .ok_or_else(|| {
                        StoreError::InvalidInput("step position is too large".to_owned())
                    })?
            }
        };

        transaction
                .execute(
                    "INSERT INTO task_steps(id, board_id, parent_task_id, idempotency_key, position, title, body, linked_task_id, required, status, created_by, created_at, updated_by, updated_at) VALUES (:id, :board_id, :parent_task_id, :idempotency_key, :position, :title, :body, :linked_task_id, :required, 'todo', :created_by, :created_at, :created_by, :created_at)",
                    (
                        (":id", input.id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":idempotency_key", idempotency_key.as_deref()),
                        (":position", position),
                        (":title", title.as_str()),
                        (":body", body.as_deref()),
                        (":linked_task_id", input.linked_task_id.as_deref()),
                        (":required", if input.required { 1_i64 } else { 0_i64 }),
                        (":created_by", input.created_by.as_str()),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;

        let plan_changed = parent.execution_plan_state != "planned";
        if plan_changed {
            transaction
                    .execute(
                        "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (:board_id, :task_id, 'planned', NULL, :actor, :updated_at) ON CONFLICT(task_id) DO UPDATE SET board_id = excluded.board_id, state = excluded.state, reason = NULL, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
                        (
                            (":board_id", board_id.as_str()),
                            (":task_id", parent.id.as_str()),
                            (":actor", input.created_by.as_str()),
                            (":updated_at", input.created_at),
                        ),
                    )
                    .await?;
            transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.execution_plan.planned', :actor, '{\"state\":\"planned\"}', :created_at)",
                        (
                            (":event_id", input.plan_event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", parent.id.as_str()),
                            (":actor", input.created_by.as_str()),
                            (":created_at", input.created_at),
                        ),
                    )
                    .await?;
        }

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.step.created', :actor, json_object('step_id', :step_id, 'linked_task_id', :linked_task_id, 'position', :position, 'required', json(CASE WHEN :required = 1 THEN 'true' ELSE 'false' END), 'status', 'todo'), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":actor", input.created_by.as_str()),
                        (":step_id", input.id.as_str()),
                        (":linked_task_id", input.linked_task_id.as_deref()),
                        (":position", position),
                        (":required", if input.required { 1_i64 } else { 0_i64 }),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;

        if matches!(
            parent.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready"
        ) {
            let dependencies_done = first_row(
                    transaction
                        .query(
                            "SELECT NOT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS dependency ON dependency.id = d.parent_task_id AND dependency.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND dependency.status NOT IN ('done', 'archived'))",
                            (
                                (":board_id", board_id.as_str()),
                                (":task_id", parent.id.as_str()),
                            ),
                        )
                        .await?,
                )
                .await?;
            let dependencies_done =
                integer_value(dependencies_done.get_value(0)?, "task_dependencies.ready")? != 0;
            let computed_target = canonical_ready_status(
                &parent.title,
                parent.description.as_deref(),
                parent.scheduled_at,
                dependencies_done,
                input.created_at,
            );
            if computed_target != input.target_status.trim() {
                return Err(StoreError::InvalidTransition(
                    "step create readiness decision is stale".to_owned(),
                ));
            }
            if computed_target != parent.status {
                let changed = transaction
                        .execute(
                            "UPDATE tasks SET status = :target_status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND lock_version = :lock_version",
                            (
                                (":target_status", computed_target),
                                (":updated_at", input.created_at),
                                (":task_id", parent.id.as_str()),
                                (":board_id", board_id.as_str()),
                                (":current_status", parent.status.as_str()),
                                (":lock_version", parent.lock_version),
                            ),
                        )
                        .await?;
                if changed != 1 {
                    return Err(StoreError::InvalidTransition(
                        "step create requires matching fresh parent task".to_owned(),
                    ));
                }
                let payload = format!(r#"{{"to_status":"{computed_target}"}}"#);
                transaction
                        .execute(
                            "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.recomputed', :actor, :payload, :created_at)",
                            (
                                (":event_id", input.recompute_event_id.as_str()),
                                (":board_id", board_id.as_str()),
                                (":task_id", parent.id.as_str()),
                                (":actor", input.created_by.as_str()),
                                (":payload", payload.as_str()),
                                (":created_at", input.created_at),
                            ),
                        )
                        .await?;
            }
        } else if input.target_status.trim() != parent.status {
            return Err(StoreError::InvalidTransition(
                "step create cannot recompute this parent status".to_owned(),
            ));
        }

        let step = step_from_row(
                &transaction,
                first_row(
                    transaction
                        .query(
                            "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :id LIMIT 1",
                            [
                                (":board_id", board_id.as_str()),
                                (":parent_task_id", parent.id.as_str()),
                                (":id", input.id.as_str()),
                            ],
                        )
                        .await?,
                )
                .await?,
            )
            .await?;
        transaction.commit().await?;
        Ok(step)
    }
}
