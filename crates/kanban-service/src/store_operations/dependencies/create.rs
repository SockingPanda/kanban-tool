use turso::transaction::TransactionBehavior;

use crate::store_operations::shared::canonical_ready_status;
use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::{create_support::*, support::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyInput {
    pub expected_child_lock_version: i64,
    pub target_child_status: String,
    pub actor: String,
    pub event_id: String,
    pub recompute_event_id: String,
    pub now: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyRecord {
    pub added: bool,
    pub dependencies: DependencySnapshotRecord,
}
impl TursoStore {
    /// Add one parent -> child dependency and return the post-mutation
    /// snapshot. The edge, optional child recomputation and event are guarded
    /// by one immediate transaction so a stale caller cannot observe a
    /// partially-applied dependency.
    pub async fn add_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: AddDependencyInput,
    ) -> Result<AddDependencyRecord, StoreError> {
        validate_add_dependency_input(child_task_id, parent_task_id, &input)?;
        let child_task_id = child_task_id.trim();
        let parent_task_id = parent_task_id.trim();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let child = dependency_task_in_transaction(&transaction, child_task_id).await?;
        let parent = dependency_task_in_transaction(&transaction, parent_task_id).await?;
        if child.board_id != parent.board_id {
            return Err(StoreError::InvalidInput(
                "cross-board dependency is not allowed".to_owned(),
            ));
        }
        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", child.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive dependencies".to_owned(),
            ));
        }
        if child.archived_at.is_some() || child.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived child task cannot receive dependencies".to_owned(),
            ));
        }

        let existing = first_row(
                transaction
                    .query(
                        "SELECT 1 FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND child_task_id = :child_task_id LIMIT 1",
                        [
                            (":board_id", child.board_id.as_str()),
                            (":parent_task_id", parent.id.as_str()),
                            (":child_task_id", child.id.as_str()),
                        ],
                    )
                    .await?,
            )
            .await;
        match existing {
            Ok(_) => {
                let dependencies = dependency_snapshot_in_transaction(
                    &transaction,
                    child.board_id.as_str(),
                    child.id.as_str(),
                )
                .await?;
                transaction.commit().await?;
                return Ok(AddDependencyRecord {
                    added: false,
                    dependencies,
                });
            }
            Err(turso::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(StoreError::Turso(error)),
        }

        if dependency_path_exists(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
            parent.id.as_str(),
        )
        .await?
        {
            return Err(StoreError::DependencyCycle(
                "dependency cycle detected".to_owned(),
            ));
        }
        if child.status == "running" && !dependency_parent_satisfied(&parent) {
            return Err(StoreError::InvalidTransition(
                "cannot add incomplete dependency to running task".to_owned(),
            ));
        }
        if input.expected_child_lock_version != child.lock_version {
            return Err(StoreError::InvalidTransition(
                "dependency add requires matching fresh child task".to_owned(),
            ));
        }

        let target_status = if matches!(
            child.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready"
        ) {
            let existing_dependencies_done = !child.dependency_blocked;
            let dependencies_done =
                existing_dependencies_done && dependency_parent_satisfied(&parent);
            let computed = canonical_ready_status(
                &child.title,
                child.description.as_deref(),
                child.scheduled_at,
                dependencies_done,
                input.now,
            );
            if computed == "ready" {
                child.status.clone()
            } else {
                computed.to_owned()
            }
        } else {
            child.status.clone()
        };
        if target_status != input.target_child_status.trim() {
            return Err(StoreError::InvalidTransition(
                "dependency add readiness decision is stale".to_owned(),
            ));
        }

        transaction
                .execute(
                    "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (:board_id, :parent_task_id, :child_task_id, :created_at) ON CONFLICT(parent_task_id, child_task_id) DO NOTHING",
                    (
                        (":board_id", child.board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":child_task_id", child.id.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        if target_status != child.status {
            let changed = transaction
                    .execute(
                        "UPDATE tasks SET status = :target_status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND archived_at IS NULL AND lock_version = :lock_version",
                        (
                            (":target_status", target_status.as_str()),
                            (":updated_at", input.now),
                            (":task_id", child.id.as_str()),
                            (":board_id", child.board_id.as_str()),
                            (":current_status", child.status.as_str()),
                            (":lock_version", input.expected_child_lock_version),
                        ),
                    )
                    .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "dependency add requires matching fresh child task".to_owned(),
                ));
            }
            let payload = format!(r#"{{"to_status":"{}"}}"#, target_status);
            transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.recomputed', :actor, :payload_json, :created_at)",
                        (
                            (":event_id", input.recompute_event_id.as_str()),
                            (":board_id", child.board_id.as_str()),
                            (":task_id", child.id.as_str()),
                            (":actor", input.actor.trim()),
                            (":payload_json", payload.as_str()),
                            (":created_at", input.now),
                        ),
                    )
                    .await?;
        }

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'dependency.added', :actor, json_object('parent_task_id', :parent_task_id), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", child.board_id.as_str()),
                        (":task_id", child.id.as_str()),
                        (":actor", input.actor.trim()),
                        (":parent_task_id", parent.id.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        let dependencies = dependency_snapshot_in_transaction(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
        )
        .await?;
        transaction.commit().await?;
        Ok(AddDependencyRecord {
            added: true,
            dependencies,
        })
    }
}
