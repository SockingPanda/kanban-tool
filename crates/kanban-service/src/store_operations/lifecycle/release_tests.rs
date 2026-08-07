#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn release_task_returns_ready_and_cancels_run_atomically() {
        let (_directory, store, _path) = store("release-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_success",
            "release-success",
            "Release success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_success",
                    "r_release_success",
                    "e_release_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");

        let released = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_success",
                    "e_release_success",
                    500,
                ),
            )
            .await
            .expect("release task");
        assert_eq!(released.id, task.id);
        assert_eq!(released.status, "ready");
        assert_eq!(released.status_reason, None);
        assert_eq!(released.claim_token, None);
        assert_eq!(released.claim_owner, None);
        assert_eq!(released.claim_expires_at, None);
        assert_eq!(released.last_heartbeat_at, None);
        assert_eq!(released.current_run_id, None);
        assert_eq!(released.started_at, claimed.task.started_at);
        assert_eq!(released.retry_count, claimed.task.retry_count);
        assert_eq!(released.updated_at, 500);
        assert_eq!(released.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_release_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "canceled"
        );
        assert_eq!(
            integer_value(
                run.get_value(1).expect("run finished_at"),
                "run.finished_at"
            )
            .expect("run finished_at integer"),
            500
        );
        assert_eq!(
            optional_text_value(run.get_value(2).expect("run error"), "run.error")
                .expect("run error text")
                .as_deref(),
            None
        );

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_release_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_release_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.released"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            500
        );
    }

    #[tokio::test]
    async fn release_task_rejects_credentials_and_guards_without_writes() {
        let (_directory, store, _path) = store("release-guards").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_guards_running",
            "release-guards-running",
            "Release guards running",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_guards",
                    "r_release_guards",
                    "e_release_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim running task");
        let connection = store.connection().await.expect("connection");

        let wrong_token = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "secret-release-token",
                    "e_release_wrong_token",
                    500,
                ),
            )
            .await
            .expect_err("wrong token must fail");
        assert!(matches!(wrong_token, StoreError::ClaimTokenMismatch));
        assert!(!wrong_token.to_string().contains("secret-release-token"));

        let padded_token = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    " claim_release_guards ",
                    "e_release_padded_token",
                    500,
                ),
            )
            .await
            .expect_err("padded token must not be normalized");
        assert!(matches!(padded_token, StoreError::ClaimTokenMismatch));

        let wrong_owner = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_release_guards",
                    "e_release_wrong_owner",
                    500,
                ),
            )
            .await
            .expect_err("wrong owner must fail");
        assert!(matches!(
            wrong_owner,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let after_credentials = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged running task");
        assert_eq!(after_credentials.status, "running");
        assert_eq!(after_credentials.lock_version, claimed.task.lock_version);
        assert_eq!(after_credentials.claim_token, claimed.task.claim_token);
        assert_eq!(
            after_credentials.current_run_id,
            claimed.task.current_run_id
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_non_running",
                    500,
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running task");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run id");
        let missing_run_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_missing_run",
                    500,
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run_error,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_release_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run id");

        let unplanned_claim = store
            .get_task_global(&task.id)
            .await
            .expect("get task before plan guard");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'unplanned' WHERE task_id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make plan unplanned");
        let unplanned_error = store
            .release_task(
                &task.id,
                release_input(
                    unplanned_claim.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_unplanned",
                    500,
                ),
            )
            .await
            .expect_err("unplanned task must not release to ready");
        assert!(matches!(
            unplanned_error,
            StoreError::InvalidTransition(message) if message.contains("execution plan")
        ));
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'not_required' WHERE task_id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore execution plan");

        let dependency_parent = store
            .create_task(
                "default",
                create_input(
                    "t_release_dependency_parent",
                    Some("release-dependency-parent"),
                    "Release dependency parent",
                ),
            )
            .await
            .expect("create dependency parent");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (dependency_parent.id.as_str(), task.id.as_str()),
            )
            .await
            .expect("insert dependency");
        let dependency_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_dependency",
                    500,
                ),
            )
            .await
            .expect_err("dependency-blocked task must not release to ready");
        assert!(matches!(
            dependency_error,
            StoreError::InvalidTransition(message) if message.contains("dependency")
        ));
        connection
            .execute(
                "DELETE FROM task_dependencies WHERE parent_task_id = ?1 AND child_task_id = ?2",
                (dependency_parent.id.as_str(), task.id.as_str()),
            )
            .await
            .expect("remove dependency");

        connection
            .execute(
                "UPDATE tasks SET scheduled_at = 1_000 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("set future schedule");
        let future_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_future",
                    500,
                ),
            )
            .await
            .expect_err("future schedule must fail");
        assert!(matches!(
            future_error,
            StoreError::InvalidTransition(message) if message.contains("future")
        ));

        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.released'",
                    (),
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("release-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_event_conflict",
            "release-event-conflict",
            "Release event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_event_conflict",
                    "r_release_event_conflict",
                    "e_release_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_release_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_event_conflict",
                    "e_release_event_conflict",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_expires_at, claimed.task.claim_expires_at);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_release_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished_at"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run error"), Value::Null));
        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.released'",
                    [task.id.as_str()],
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("release-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_release_input", "release-input", "Release input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                release_input(1, "worker", "claim_input", "e_input", 500),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                release_input(-1, "worker", "claim_input", "e_input_version", 500),
            ),
            (
                "actor",
                task.id.clone(),
                release_input(1, " ", "claim_input", "e_input_actor", 500),
            ),
            (
                "claim_token",
                task.id.clone(),
                release_input(1, "worker", " ", "e_input_token", 500),
            ),
            (
                "event_id",
                task.id.clone(),
                release_input(1, "worker", "claim_input", "input_event", 500),
            ),
            (
                "now",
                task.id.clone(),
                release_input(1, "worker", "claim_input", "e_input_now", -1),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .release_task(&task_id, input)
                .await
                .expect_err("invalid release input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.released'",
                    (),
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("release-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_release_other', 'release-other', 'Release other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "release-other",
                create_input(
                    "t_release_other",
                    Some("release-other"),
                    "Release other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board release plan",
                    "planner",
                    "e_release_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_release_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_release_other",
                    "r_release_other",
                    "e_release_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let released = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_release_other",
                    "e_release_other",
                    500,
                ),
            )
            .await
            .expect("release other-board task");
        assert_eq!(released.board_id, "b_release_other");
        assert_eq!(released.board_slug, "release-other");

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_release_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_release_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_release_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
    }
}
