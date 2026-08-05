use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, TaskStepsRecord};

pub trait StepList: ApplicationStore {
    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: StepList,
    C: Clock,
{
    pub async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.list_steps(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::StubStore;
    use crate::*;

    impl StepList for StubStore {
        async fn list_steps(&self, _task_id: &str) -> Result<TaskStepsRecord> {
            Err(KanbanError::FeatureNotAvailable(
                "step stub is not configured".to_owned(),
            ))
        }
    }
}
