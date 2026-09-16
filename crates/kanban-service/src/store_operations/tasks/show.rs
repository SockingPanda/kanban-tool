use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

impl TursoStore {
    pub async fn get_task_global(&self, task_id: &str) -> Result<TaskRecord, StoreError> {
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "task id must start with t_".to_owned(),
            ));
        }
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(turso::transaction::TransactionBehavior::Deferred)
            .await?;
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
        let mut task = task_from_row(row)?;
        task.labels = crate::store_operations::labels::list_task_labels_in_transaction(
            &transaction,
            &task.board_id,
            task_id,
        )
        .await?;
        transaction.commit().await?;
        Ok(task)
    }
}
