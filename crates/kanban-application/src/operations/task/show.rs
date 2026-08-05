use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.get_task(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::KanbanError;

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;
    #[tokio::test]
    async fn get_task_accepts_only_global_task_ids() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let task = service.get_task(" t_show ").await.unwrap();
        assert_eq!(task.id, "t_show");

        let error = service.get_task("default#1").await.unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}
