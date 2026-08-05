use crate::{db::TursoStore, domain::TaskRunRecord, error::StoreError};

impl TursoStore {
    /// Return one run by its global id without mutating canonical state.
    pub async fn get_run(&self, run_id: &str) -> Result<TaskRunRecord, StoreError> {
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
    async fn get_run_rejects_non_global_ids() {
        let (_directory, store, _path) = store("run-show-invalid").await;
        store.initialize().await.expect("initialize");

        for run_id in ["", " ", "run_123", "t_task", "r_"] {
            let error = store
                .get_run(run_id)
                .await
                .expect_err("invalid run id must be rejected");
            assert!(
                matches!(error, StoreError::InvalidInput(message) if message.contains("run id"))
            );
        }
    }

    #[tokio::test]
    async fn get_run_returns_exact_record_and_does_not_mutate_task_run_or_event() {
        let (_directory, store, _path) = store("run-show-exact").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(&store, "t_run_show", "run-show", "Run show").await;
        let mut input = claim_input(
            1,
            "worker-show",
            "claim-show",
            "r_run_show",
            "e_run_show_claim",
            r#"{"source":"run-show"}"#,
            300,
            1_000,
        );
        input.log_path = Some(" /tmp/run-show.log ".to_owned());
        let claimed = store.claim_task(&task.id, input).await.expect("claim task");

        let task_before = store
            .get_task_global(&task.id)
            .await
            .expect("read task before show");
        let run_before = store
            .get_run(" r_run_show ")
            .await
            .expect("read run before show");
        assert_eq!(run_before, claimed.run);
        assert_eq!(run_before.id, "r_run_show");
        assert_eq!(run_before.board_id, "b_default");
        assert_eq!(run_before.task_id, task.id);
        assert_eq!(run_before.status, "running");
        assert_eq!(run_before.worker_profile.as_deref(), Some("manual"));
        assert_eq!(run_before.worker_pid, None);
        assert_eq!(run_before.claim_token, "claim-show");
        assert_eq!(run_before.claim_owner, "worker-show");
        assert_eq!(run_before.claim_expires_at, 1_300);
        assert_eq!(run_before.started_at, 300);
        assert_eq!(run_before.last_heartbeat_at, Some(300));
        assert_eq!(run_before.finished_at, None);
        assert_eq!(run_before.exit_code, None);
        assert_eq!(run_before.summary, None);
        assert_eq!(run_before.error, None);
        assert_eq!(run_before.log_path.as_deref(), Some("/tmp/run-show.log"));
        assert_eq!(run_before.metadata_json, r#"{"source":"run-show"}"#);

        let connection = store.connection().await.expect("connection");
        let event_before = first_row(
            connection
                .query(
                    "SELECT event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_run_show_claim"],
                )
                .await
                .expect("event before query"),
        )
        .await
        .expect("event before row");

        let shown = store.get_run("r_run_show").await.expect("show run");
        assert_eq!(shown, run_before);

        let task_after = store
            .get_task_global(&task.id)
            .await
            .expect("read task after show");
        let run_after = store
            .get_run("r_run_show")
            .await
            .expect("read run after show");
        assert_eq!(task_after, task_before);
        assert_eq!(run_after, run_before);

        let event_after = first_row(
            connection
                .query(
                    "SELECT event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_run_show_claim"],
                )
                .await
                .expect("event after query"),
        )
        .await
        .expect("event after row");
        for index in 0..8 {
            assert_eq!(
                event_after.get_value(index).expect("event after value"),
                event_before.get_value(index).expect("event before value")
            );
        }
    }

    #[tokio::test]
    async fn get_run_preserves_run_not_found_error() {
        let (_directory, store, _path) = store("run-show-missing").await;
        store.initialize().await.expect("initialize");

        let error = store
            .get_run(" r_missing ")
            .await
            .expect_err("unknown run must be missing");
        assert!(matches!(error, StoreError::RunNotFound(run_id) if run_id == "r_missing"));
    }
}
