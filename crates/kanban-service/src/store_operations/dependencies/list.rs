use crate::store_operations::shared::validate_task_id;
use crate::{db::TursoStore, domain::*, error::StoreError};

use super::support::*;

impl TursoStore {
    pub async fn list_dependencies(
        &self,
        task_id: &str,
    ) -> Result<DependencySnapshotRecord, StoreError> {
        validate_task_id(task_id)?;
        let task_id = task_id.trim();
        let connection = self.connection().await?;
        let task = dependency_task_in_connection(&connection, task_id).await?;
        dependency_snapshot_in_connection(&connection, task.board_id.as_str(), task.id.as_str())
            .await
    }
}
