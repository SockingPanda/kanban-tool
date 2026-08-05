use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::{remove_support::*, support::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyInput {
    pub actor: String,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyRecord {
    pub removed: bool,
    pub dependencies: DependencySnapshotRecord,
}
impl TursoStore {
    /// Remove one parent -> child dependency and return the post-mutation
    /// snapshot. The edge delete and its event are guarded by one immediate
    /// transaction; a missing edge is a successful no-op with no event.
    pub async fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: RemoveDependencyInput,
    ) -> Result<RemoveDependencyRecord, StoreError> {
        validate_remove_dependency_input(child_task_id, parent_task_id, &input)?;
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
                "archived board cannot remove dependencies".to_owned(),
            ));
        }
        if child.archived_at.is_some() || child.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived child task cannot remove dependencies".to_owned(),
            ));
        }

        let deleted = transaction
                .execute(
                    "DELETE FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND child_task_id = :child_task_id",
                    [
                        (":board_id", child.board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":child_task_id", child.id.as_str()),
                    ],
                )
                .await?;
        if deleted == 1 {
            transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'dependency.removed', :actor, json_object('parent_task_id', :parent_task_id), :created_at)",
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
        }

        let dependencies = dependency_snapshot_in_transaction(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
        )
        .await?;
        transaction.commit().await?;
        Ok(RemoveDependencyRecord {
            removed: deleted == 1,
            dependencies,
        })
    }
}
