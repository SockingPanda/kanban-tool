use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore};

/// Maximum bytes exposed by the future run-log reader.
///
/// The canonical contract deliberately has no tail query; readers always use
/// this bounded suffix when the operation is implemented.
pub const RUN_LOG_TAIL_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLogRecord {
    pub run_id: String,
    pub content: String,
    pub truncated: bool,
}

pub trait RunLog: ApplicationStore {
    fn read_run_log(
        &self,
        run_id: &str,
        max_bytes: usize,
    ) -> impl Future<Output = Result<RunLogRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: RunLog,
    C: Clock,
{
    pub async fn get_run_log(&self, run_id: &str) -> Result<RunLogRecord> {
        let run_id = run_id.trim();
        if !run_id.starts_with("r_") || run_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "run_id must be a global r_... id".to_owned(),
            ));
        }

        self.store.read_run_log(run_id, RUN_LOG_TAIL_BYTES).await
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

    impl RunLog for StubStore {
        async fn read_run_log(&self, run_id: &str, max_bytes: usize) -> Result<RunLogRecord> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(max_bytes, RUN_LOG_TAIL_BYTES);

            match run_id {
                "r_log" => Ok(RunLogRecord {
                    run_id: run_id.to_owned(),
                    content: "tail".to_owned(),
                    truncated: true,
                }),
                "r_error" => Err(KanbanError::Storage("read failed".to_owned())),
                _ => panic!("unexpected run id: {run_id}"),
            }
        }
    }

    fn service(calls: Arc<AtomicUsize>) -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(StubStore { calls }, FixedClock(100))
    }

    #[tokio::test]
    async fn get_run_log_requires_a_global_run_id_without_calling_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());

        let error = service
            .get_run_log(" default#1 ")
            .await
            .expect_err("board-local selectors must be rejected by the application");

        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn get_run_log_trims_id_uses_fixed_tail_and_preserves_result() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());

        let result = service.get_run_log(" r_log ").await.unwrap();

        assert_eq!(
            result,
            RunLogRecord {
                run_id: "r_log".to_owned(),
                content: "tail".to_owned(),
                truncated: true,
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn get_run_log_preserves_store_error_and_does_not_take_mutation_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());
        let _mutation_guard = service.mutation_gate.lock().await;

        let result = tokio::time::timeout(Duration::from_secs(1), service.get_run_log("r_error"))
            .await
            .expect("queries must not wait for the mutation gate");

        assert!(matches!(result, Err(KanbanError::Storage(message)) if message == "read failed"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
