use crate::{db::TursoStore, domain::TaskRunRecord, error::StoreError};

impl TursoStore {
    /// 返回 server 提供日志时使用的规范 run record。
    ///
    /// store 有意不检查或打开 `log_path`；通过此 query 加载 run 后，这属于 server 侧职责。
    pub async fn get_run_log_source(&self, run_id: &str) -> Result<TaskRunRecord, StoreError> {
        let run_id = run_id.trim();
        if !run_id.starts_with("r_") || run_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "run id must start with r_".to_owned(),
            ));
        }

        super::shared::load_run(self, run_id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn get_run_log_source_rejects_invalid_ids_without_writes() {
        let (_directory, store, _path) = store("run-log-invalid").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        let task_runs_before = count_rows(&connection, "task_runs").await;
        let events_before = count_rows(&connection, "task_events").await;

        for run_id in ["", "r_", " r_ ", "task-local", "default#1"] {
            let error = store
                .get_run_log_source(run_id)
                .await
                .expect_err("invalid run id must be rejected");
            assert!(
                matches!(error, StoreError::InvalidInput(message) if message.contains("run id"))
            );
        }

        assert_eq!(count_rows(&connection, "task_runs").await, task_runs_before);
        assert_eq!(count_rows(&connection, "task_events").await, events_before);
    }

    #[tokio::test]
    async fn get_run_log_source_returns_exact_log_path_and_preserves_state() {
        let (_directory, store, _path) = store("run-log-some").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(&store, "t_run_log_some", "run-log-some", "Run log").await;
        let mut input = claim_input(
            1,
            "worker",
            "run_log_some_token",
            "r_run_log_some",
            "e_run_log_some",
            "{}",
            300,
            1_000,
        );
        input.log_path = Some(" /tmp/kanban-run.log ".to_owned());
        let claimed = store.claim_task(&task.id, input).await.expect("claim task");
        assert_eq!(claimed.run.log_path.as_deref(), Some("/tmp/kanban-run.log"));

        let before_task = store
            .get_task_global(&task.id)
            .await
            .expect("task before log lookup");
        let connection = store.connection().await.expect("connection");
        let runs_before = count_rows(&connection, "task_runs").await;
        let events_before = count_rows(&connection, "task_events").await;

        let loaded = store
            .get_run_log_source("  r_run_log_some  ")
            .await
            .expect("load run log source");
        assert_eq!(loaded, claimed.run);

        let after_task = store
            .get_task_global(&task.id)
            .await
            .expect("task after log lookup");
        assert_eq!(after_task, before_task);
        assert_eq!(count_rows(&connection, "task_runs").await, runs_before);
        assert_eq!(count_rows(&connection, "task_events").await, events_before);
    }

    #[tokio::test]
    async fn get_run_log_source_returns_none_log_path_and_preserves_state() {
        let (_directory, store, _path) = store("run-log-none").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(&store, "t_run_log_none", "run-log-none", "Run log").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "run_log_none_token",
                    "r_run_log_none",
                    "e_run_log_none",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        assert_eq!(claimed.run.log_path, None);

        let before_task = store
            .get_task_global(&task.id)
            .await
            .expect("task before log lookup");
        let connection = store.connection().await.expect("connection");
        let runs_before = count_rows(&connection, "task_runs").await;
        let events_before = count_rows(&connection, "task_events").await;

        let loaded = store
            .get_run_log_source(&claimed.run.id)
            .await
            .expect("load run log source");
        assert_eq!(loaded, claimed.run);
        assert_eq!(loaded.log_path, None);

        let after_task = store
            .get_task_global(&task.id)
            .await
            .expect("task after log lookup");
        assert_eq!(after_task, before_task);
        assert_eq!(count_rows(&connection, "task_runs").await, runs_before);
        assert_eq!(count_rows(&connection, "task_events").await, events_before);
    }

    #[tokio::test]
    async fn get_run_log_source_returns_run_not_found_without_writes() {
        let (_directory, store, _path) = store("run-log-missing").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        let runs_before = count_rows(&connection, "task_runs").await;
        let events_before = count_rows(&connection, "task_events").await;

        let error = store
            .get_run_log_source("  r_missing_log  ")
            .await
            .expect_err("missing run must be not found");
        assert!(matches!(
            error,
            StoreError::RunNotFound(run_id) if run_id == "r_missing_log"
        ));
        assert_eq!(count_rows(&connection, "task_runs").await, runs_before);
        assert_eq!(count_rows(&connection, "task_events").await, events_before);
    }
}
