#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn submit_review_task_moves_running_task_and_run_atomically() {
        let (_directory, store, _path) = store("review-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_review_success",
            "review-success",
            "Review success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_success",
                    "r_review_success",
                    "e_review_claim",
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
                "UPDATE task_runs SET error = ?1 WHERE id = ?2",
                ("preexisting error", "r_review_success"),
            )
            .await
            .expect("set preexisting run error");

        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_success"),
                    false,
                    Some("ready for review"),
                    500,
                    "e_review_success",
                ),
            )
            .await
            .expect("submit review");
        assert_eq!(reviewed.id, task.id);
        assert_eq!(reviewed.status, "review");
        assert_eq!(reviewed.status_reason, None);
        assert_eq!(reviewed.claim_token, None);
        assert_eq!(reviewed.claim_owner, None);
        assert_eq!(reviewed.claim_expires_at, None);
        assert_eq!(reviewed.last_heartbeat_at, None);
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_success"));
        assert_eq!(reviewed.result_summary.as_deref(), Some("ready for review"));
        assert_eq!(reviewed.completed_at, None);
        assert_eq!(reviewed.updated_at, 500);
        assert_eq!(reviewed.lock_version, claimed.task.lock_version + 1);

        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary, error FROM task_runs WHERE id = ?1",
                    ["r_review_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "succeeded"
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
            integer_value(run.get_value(2).expect("run exit_code"), "run.exit_code")
                .expect("run exit code integer"),
            0
        );
        assert_eq!(
            optional_text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text")
                .as_deref(),
            Some("ready for review")
        );
        assert_eq!(
            optional_text_value(run.get_value(4).expect("run error"), "run.error")
                .expect("run error text")
                .as_deref(),
            Some("preexisting error")
        );

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_review_success"],
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
            "r_review_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.submitted_for_review"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
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
    async fn submit_review_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("review-guards").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_guards", "review-guards", "Review guards").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_guards",
                    "r_review_guards",
                    "e_review_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-review-token"), "e_review_wrong_token"),
            (Some(" claim_review_guards "), "e_review_padded_token"),
            (None, "e_review_missing_token"),
        ] {
            let error = store
                .submit_review_task(
                    &task.id,
                    submit_review_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        None,
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-review-token"));
        }

        let owner_error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_non_running",
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
            .expect("remove current run");
        let missing_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_missing_run",
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_review_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("tamper run owner");
        let inconsistent_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_inconsistent_run",
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent_run,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'worker' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("restore run owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("restore active run");

        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.submitted_for_review'",
                    [task.id.as_str()],
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
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
    async fn submit_review_task_force_bypasses_input_credentials_but_keeps_run_consistency() {
        let (_directory, store, _path) = store("review-force").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_force", "review-force", "Review force").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_force",
                    "r_review_force",
                    "e_review_force_claim",
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
                "UPDATE tasks SET result_summary = ?1 WHERE id = ?2",
                ("existing task summary", task.id.as_str()),
            )
            .await
            .expect("set existing task summary");
        connection
            .execute(
                "UPDATE task_runs SET summary = ?1 WHERE id = ?2",
                ("existing run summary", "r_review_force"),
            )
            .await
            .expect("set existing run summary");

        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "force-reviewer",
                    Some("wrong force token"),
                    true,
                    None,
                    500,
                    "e_review_force",
                ),
            )
            .await
            .expect("force submit review");
        assert_eq!(reviewed.status, "review");
        assert_eq!(
            reviewed.result_summary.as_deref(),
            Some("existing task summary")
        );
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_force"));

        let run = first_row(
            connection
                .query(
                    "SELECT summary FROM task_runs WHERE id = ?1",
                    ["r_review_force"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            optional_text_value(run.get_value(0).expect("run summary"), "run.summary")
                .expect("run summary text")
                .as_deref(),
            Some("existing run summary")
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_review_force"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "force-reviewer"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn submit_review_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("review-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_review_event_conflict",
            "review-event-conflict",
            "Review event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_event_conflict",
                    "r_review_event_conflict",
                    "e_review_event_claim",
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
                ("e_review_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_event_conflict"),
                    false,
                    Some("must roll back"),
                    500,
                    "e_review_event_conflict",
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
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary FROM task_runs WHERE id = ?1",
                    ["r_review_event_conflict"],
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
        assert!(matches!(
            run.get_value(2).expect("run exit_code"),
            Value::Null
        ));
        assert!(matches!(
            run.get_value(3).expect("run summary"),
            Value::Null
        ));
        let review_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.submitted_for_review'",
                    [task.id.as_str()],
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
        assert_eq!(
            integer_value(
                review_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn submit_review_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("review-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_input", "review-input", "Review input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                submit_review_input(1, "worker", Some("claim"), false, None, 500, "e_input"),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                submit_review_input(
                    -1,
                    "worker",
                    Some("claim"),
                    false,
                    None,
                    500,
                    "e_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                submit_review_input(1, " ", Some("claim"), false, None, 500, "e_input_actor"),
            ),
            (
                "event_id",
                task.id.clone(),
                submit_review_input(1, "worker", Some("claim"), false, None, 500, "input_event"),
            ),
            (
                "now",
                task.id.clone(),
                submit_review_input(1, "worker", Some("claim"), false, None, -1, "e_input_now"),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .submit_review_task(&task_id, input)
                .await
                .expect_err("invalid review input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        let review_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.submitted_for_review'",
                    (),
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
        assert_eq!(
            integer_value(
                review_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn submit_review_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("review-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_review_other', 'review-other', 'Review other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "review-other",
                create_input("t_review_other", Some("review-other"), "Review other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board review plan",
                    "planner",
                    "e_review_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_review_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_review_other",
                    "r_review_other",
                    "e_review_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_review_other"),
                    false,
                    None,
                    500,
                    "e_review_other",
                ),
            )
            .await
            .expect("review other-board task");
        assert_eq!(reviewed.board_id, "b_review_other");
        assert_eq!(reviewed.board_slug, "review-other");
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_other"));

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_review_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_review_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_review_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }
}
