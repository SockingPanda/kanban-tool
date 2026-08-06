use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, RunRecord};

/// run history query 所需的持久化 capability。
pub trait RunList: ApplicationStore {
    fn list_runs(&self, task_id: &str) -> impl Future<Output = Result<Vec<RunRecord>>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: RunList,
    C: Clock,
{
    pub async fn list_runs(&self, task_id: &str) -> Result<Vec<RunRecord>> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.list_runs(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    fn run(id: &str, task_id: &str, started_at: i64) -> RunRecord {
        RunRecord {
            id: id.into(),
            board_id: "b_default".into(),
            task_id: task_id.into(),
            status: RunStatus::Succeeded,
            worker_profile: Some("test-worker".into()),
            worker_pid: Some(42),
            claim_owner: "worker".into(),
            claim_expires_at: started_at + 100,
            started_at,
            last_heartbeat_at: Some(started_at + 1),
            finished_at: Some(started_at + 2),
            exit_code: Some(0),
            summary: Some(format!("run {id}")),
            error: None,
            log_path: Some(format!("/tmp/{id}.log")),
            metadata_json: "{}".into(),
        }
    }

    impl RunList for StubStore {
        async fn list_runs(&self, task_id: &str) -> Result<Vec<RunRecord>> {
            assert_eq!(task_id, "t_history");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![run("r_new", task_id, 200), run("r_old", task_id, 100)])
        }
    }

    fn service(calls: Arc<AtomicUsize>) -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(StubStore { calls }, FixedClock(100))
    }

    #[tokio::test]
    async fn list_runs_rejects_non_global_ids_without_calling_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());

        for selector in ["", "t_", "default#1", "task-1"] {
            let error = service.list_runs(selector).await.unwrap_err();
            assert!(matches!(error, KanbanError::InvalidInput(_)));
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_runs_trims_id_calls_store_once_and_preserves_order() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());

        let records = service.list_runs("  t_history  ").await.unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["r_new", "r_old"]
        );
        assert_eq!(records[0].task_id, "t_history");
        assert_eq!(records[1].started_at, 100);
    }

    #[tokio::test]
    async fn list_runs_does_not_wait_for_the_mutation_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls);
        let _mutation = service.mutation_gate.lock().await;

        let result =
            tokio::time::timeout(Duration::from_millis(100), service.list_runs("t_history"))
                .await
                .expect("query must not wait for mutation gate")
                .expect("run history query");

        assert_eq!(result.len(), 2);
    }
}
