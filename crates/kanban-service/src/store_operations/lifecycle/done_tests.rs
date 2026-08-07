#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn complete_task_writes_done_run_and_result_event() {
        let (_directory, store, _path) = store("complete-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_success",
            "complete-success",
            "Complete success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_success",
                    "r_complete_success",
                    "e_complete_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let setup_connection = store.connection().await.expect("connection");
        setup_connection
            .execute(
                "UPDATE task_runs SET error = ?1 WHERE id = ?2",
                ("preexisting error", "r_complete_success"),
            )
            .await
            .expect("set preexisting run error");

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_success"),
                    false,
                    Some("finished"),
                    Some(r#"{"ok":true}"#),
                    500,
                    "e_complete_success",
                ),
            )
            .await
            .expect("complete task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.status_reason, None);
        assert_eq!(completed.completed_at, Some(500));
        assert_eq!(completed.claim_token, None);
        assert_eq!(completed.claim_owner, None);
        assert_eq!(completed.claim_expires_at, None);
        assert_eq!(completed.last_heartbeat_at, None);
        assert_eq!(
            completed.current_run_id.as_deref(),
            Some("r_complete_success")
        );
        assert_eq!(completed.result_summary.as_deref(), Some("finished"));
        assert_eq!(completed.result_json.as_deref(), Some(r#"{"ok":true}"#));
        assert_eq!(completed.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary, error FROM task_runs WHERE id = ?1",
                    ["r_complete_success"],
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
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text"),
            "finished"
        );
        assert!(matches!(run.get_value(4).expect("run error"), Value::Null));

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_complete_success"],
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
            "r_complete_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.completed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":{"ok":true}}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
    }

    #[tokio::test]
    async fn complete_task_review_does_not_require_token_or_finish_succeeded_run() {
        let (_directory, store, _path) = store("complete-review").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_review",
            "complete-review",
            "Complete review",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_review",
                    "r_complete_review",
                    "e_complete_review_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_review"),
                    false,
                    Some("reviewed"),
                    400,
                    "e_complete_review_submit",
                ),
            )
            .await
            .expect("submit review");
        assert_eq!(reviewed.status, "review");
        assert_eq!(reviewed.result_summary.as_deref(), Some("reviewed"));

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    reviewed.lock_version,
                    "reviewer",
                    None,
                    false,
                    None,
                    None,
                    500,
                    "e_complete_review_done",
                ),
            )
            .await
            .expect("complete reviewed task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.result_summary.as_deref(), Some("reviewed"));
        assert_eq!(
            completed.current_run_id.as_deref(),
            Some("r_complete_review")
        );
        assert_eq!(completed.completed_at, Some(500));

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code FROM task_runs WHERE id = ?1",
                    ["r_complete_review"],
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
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            400
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        let event = first_row(
            connection
                .query(
                    "SELECT run_id, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_review_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_complete_review"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "reviewer"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn complete_task_rejects_credentials_steps_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("complete-guards").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_guards",
            "complete-guards",
            "Complete guards",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_guards",
                    "r_complete_guards",
                    "e_complete_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-complete-token"), "e_complete_wrong_token"),
            (Some(" claim_complete_guards "), "e_complete_padded_token"),
            (None, "e_complete_missing_token"),
        ] {
            let error = store
                .complete_task(
                    &task.id,
                    complete_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        None,
                        None,
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-complete-token"));
        }

        let owner_error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_non_running",
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running or review")
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
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_missing_run",
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
                ("r_complete_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("tamper run owner");
        let inconsistent_run = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_inconsistent_run",
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
                ["r_complete_guards"],
            )
            .await
            .expect("restore run owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("restore active run");

        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for damaged state");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove task claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks after damaged state");
        let missing_claim_expiry = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "admin",
                    None,
                    true,
                    None,
                    None,
                    500,
                    "e_complete_missing_claim_expiry",
                ),
            )
            .await
            .expect_err("missing claim expiry must fail");
        assert!(matches!(
            missing_claim_expiry,
            StoreError::InvalidTransition(message) if message.contains("active claim")
        ));
        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for restore");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = ?1 WHERE id = ?2",
                (1_300_i64, task.id.as_str()),
            )
            .await
            .expect("restore task claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks");

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_archived",
                ),
            )
            .await
            .expect_err("archived task must fail");
        assert!(matches!(
            archived,
            StoreError::InvalidTransition(message) if message.contains("archived")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running', archived_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore task archive state");

        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'Required', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_complete_required", task.id.as_str()),
            )
            .await
            .expect("insert incomplete required step");
        let incomplete = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "unrelated",
                    Some("wrong-token"),
                    true,
                    None,
                    None,
                    500,
                    "e_complete_steps_incomplete",
                ),
            )
            .await
            .expect_err("incomplete required step must fail even force");
        assert!(matches!(incomplete, StoreError::StepsIncomplete(_)));
        connection
            .execute(
                "DELETE FROM task_steps WHERE id = ?1",
                ["step_complete_required"],
            )
            .await
            .expect("remove required step");

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_force_bypasses_caller_credentials_and_preserves_results() {
        let (_directory, store, _path) = store("complete-force").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_force",
            "complete-force",
            "Complete force",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_force",
                    "r_complete_force",
                    "e_complete_force_claim",
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
                "UPDATE tasks SET result_summary = ?1, result_json = ?2 WHERE id = ?3",
                ("previous result", r#"{"previous":true}"#, task.id.as_str()),
            )
            .await
            .expect("set previous task result");
        connection
            .execute(
                "UPDATE task_runs SET summary = ?1 WHERE id = ?2",
                ("previous run summary", "r_complete_force"),
            )
            .await
            .expect("set previous run summary");

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "admin",
                    Some("wrong-token"),
                    true,
                    None,
                    None,
                    500,
                    "e_complete_force_done",
                ),
            )
            .await
            .expect("force complete task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.result_summary.as_deref(), Some("previous result"));
        assert_eq!(
            completed.result_json.as_deref(),
            Some(r#"{"previous":true}"#)
        );

        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary FROM task_runs WHERE id = ?1",
                    ["r_complete_force"],
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
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text"),
            "previous run summary"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_force_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "admin"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn complete_task_event_conflict_rolls_back_task_and_run() {
        let (_directory, store, _path) = store("complete-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_event_conflict",
            "complete-event-conflict",
            "Complete event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_event_conflict",
                    "r_complete_event_conflict",
                    "e_complete_event_claim",
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
                ("e_complete_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_event_conflict"),
                    false,
                    Some("should rollback"),
                    Some(r#"{"ok":true}"#),
                    500,
                    "e_complete_event_conflict",
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
                    ["r_complete_event_conflict"],
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
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run exit"), Value::Null));
        assert!(matches!(
            run.get_value(3).expect("run summary"),
            Value::Null
        ));
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_validates_input_and_result_json_without_writes() {
        let (_directory, store, _path) = store("complete-input").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_input",
            "complete-input",
            "Complete input",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_input",
                    "r_complete_input",
                    "e_complete_input_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_task",
                ),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                complete_input(
                    -1,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    " ",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_actor",
                ),
            ),
            (
                "event_id",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "invalid_event",
                ),
            ),
            (
                "now",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    -1,
                    "e_complete_input_now",
                ),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .complete_task(&task_id, input)
                .await
                .expect_err("invalid complete input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }

        let invalid_json = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    Some("{not-json"),
                    500,
                    "e_complete_invalid_json",
                ),
            )
            .await
            .expect_err("invalid result json must fail");
        assert!(matches!(
            invalid_json,
            StoreError::InvalidInput(message) if message.contains("result_json")
        ));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let connection = store.connection().await.expect("connection");
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("complete-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_complete_other', 'complete-other', 'Complete other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "complete-other",
                create_input(
                    "t_complete_other",
                    Some("complete-other"),
                    "Complete other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board completion plan",
                    "planner",
                    "e_complete_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_complete_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_other",
                    "r_complete_other",
                    "e_complete_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_other"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_other_done",
                ),
            )
            .await
            .expect("complete other-board task");
        assert_eq!(completed.board_id, "b_complete_other");
        assert_eq!(completed.board_slug, "complete-other");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_other_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_complete_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_complete_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.completed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }
}
