use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, RunRecord};

/// Storage capability required by the `run.show` query.
pub trait RunShow: ApplicationStore {
    fn get_run(&self, run_id: &str) -> impl Future<Output = Result<RunRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: RunShow,
    C: Clock,
{
    /// Return one run by its canonical global id.
    ///
    /// Selectors such as `default#1` are deliberately rejected here; this
    /// operation only accepts the canonical `r_...` id.
    pub async fn get_run(&self, run_id: &str) -> Result<RunRecord> {
        let run_id = run_id.trim();
        if !run_id.starts_with("r_") || run_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "run_id must be a global r_... id".to_owned(),
            ));
        }
        self.store.get_run(run_id).await
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

    use super::RunShow;
    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::{ApplicationService, RunRecord, RunStatus};

    impl RunShow for StubStore {
        async fn get_run(&self, run_id: &str) -> Result<RunRecord> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match run_id {
                "r_missing" => Err(KanbanError::NotFound("run r_missing".to_owned())),
                "r_error" => Err(KanbanError::Storage("backend unavailable".to_owned())),
                _ => Ok(RunRecord {
                    id: run_id.to_owned(),
                    board_id: "b_default".into(),
                    task_id: "t_task".into(),
                    status: RunStatus::Running,
                    worker_profile: Some("worker".into()),
                    worker_pid: Some(42),
                    claim_owner: "alice".into(),
                    claim_expires_at: 200,
                    started_at: 100,
                    last_heartbeat_at: Some(150),
                    finished_at: None,
                    exit_code: None,
                    summary: Some("running".into()),
                    error: None,
                    log_path: Some("logs/r_show.log".into()),
                    metadata_json: r#"{"source":"test"}"#.into(),
                }),
            }
        }
    }

    fn service(calls: Arc<AtomicUsize>) -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(StubStore { calls }, FixedClock(100))
    }

    #[tokio::test]
    async fn get_run_rejects_non_global_id_without_calling_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let error = service(Arc::clone(&calls))
            .get_run("default#1")
            .await
            .expect_err("board-local run selectors must be rejected");

        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn get_run_trims_id_returns_exact_record_and_calls_store_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let record = service(Arc::clone(&calls))
            .get_run("  r_show  ")
            .await
            .expect("trimmed global id should be forwarded");

        assert_eq!(record.id, "r_show");
        assert_eq!(record.task_id, "t_task");
        assert_eq!(record.metadata_json, r#"{"source":"test"}"#);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn get_run_preserves_errors_without_taking_mutation_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(Arc::clone(&calls));
        let _mutation = service.mutation_gate.lock().await;

        let missing =
            tokio::time::timeout(Duration::from_millis(100), service.get_run("r_missing"))
                .await
                .expect("queries must not wait for the mutation gate")
                .expect_err("missing runs must remain not_found");
        assert!(matches!(missing, KanbanError::NotFound(message) if message == "run r_missing"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        drop(_mutation);
        let storage = service
            .get_run("r_error")
            .await
            .expect_err("storage errors must be returned unchanged");
        assert!(
            matches!(storage, KanbanError::Storage(message) if message == "backend unavailable")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
