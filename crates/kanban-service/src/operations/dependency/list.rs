use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, DependencySnapshotRecord};

pub trait DependencyList: ApplicationStore {
    fn list_dependencies(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<DependencySnapshotRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: DependencyList,
    C: Clock,
{
    pub async fn list_dependencies(&self, task_id: &str) -> Result<DependencySnapshotRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.list_dependencies(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::StubStore;
    use crate::*;

    impl DependencyList for StubStore {
        async fn list_dependencies(&self, _task_id: &str) -> Result<DependencySnapshotRecord> {
            Err(KanbanError::FeatureNotAvailable(
                "dependency stub is not configured".to_owned(),
            ))
        }
    }
}
