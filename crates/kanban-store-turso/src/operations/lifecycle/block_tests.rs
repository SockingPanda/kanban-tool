#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn block_task_ready_writes_blocked_task_and_reason_event() {
        let (_directory, store, _path) = store("block-ready").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_ready", "block-ready", "Block ready").await;
        let reason = "waiting on API";
        let blocked = store
            .block_task(
                &task.id,
                block_input(1, " blocker ", None, false, reason, 500, "e_block_ready"),
            )
            .await
            .expect("block ready task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some(reason));
        assert_eq!(blocked.lock_version, 2);
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.claim_expires_at, None);
        assert_eq!(blocked.last_heartbeat_at, None);
        assert_eq!(blocked.current_run_id, None);

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_block_ready"],
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
        assert!(matches!(
            event.get_value(2).expect("event run"),
            Value::Null
        ));
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.blocked"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "blocker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"waiting on API"}"#
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
    async fn block_task_running_fails_run_and_clears_claim_atomically() {
        let (_directory, store, _path) = store("block-running").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_running", "block-running", "Block running").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_running",
                    "r_block_running",
                    "e_block_running_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let reason = "worker failed: waiting for API";
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_running"),
                    false,
                    reason,
                    500,
                    "e_block_running",
                ),
            )
            .await
            .expect("block running task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some(reason));
        assert_eq!(blocked.completed_at, None);
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_running"));
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.claim_expires_at, None);
        assert_eq!(blocked.last_heartbeat_at, None);
        assert_eq!(blocked.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_running"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "failed"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            1
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run error"), "run.error").expect("run error text"),
            reason
        );
        let event = first_row(
            connection
                .query(
                    "SELECT run_id, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_running"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_block_running"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"worker failed: waiting for API"}"#
        );
    }

    #[tokio::test]
    async fn block_task_accepts_all_public_non_running_source_states_and_review() {
        let (_directory, store, _path) = store("block-sources").await;
        store.initialize().await.expect("initialize");
        for (index, status, event_id) in [
            (1_i64, "triage", "e_block_triage"),
            (2_i64, "todo", "e_block_todo"),
            (3_i64, "scheduled", "e_block_scheduled"),
        ] {
            let task_id = format!("t_block_source_{index}");
            let mut task_input = create_input(
                &task_id,
                Some(&format!("block-source-{index}")),
                "Block source",
            );
            task_input.status = status.to_owned();
            let task = store
                .create_task("default", task_input)
                .await
                .expect("create source task");
            let blocked = store
                .block_task(
                    &task.id,
                    block_input(0, "worker", None, false, "waiting", 500, event_id),
                )
                .await
                .expect("block source task");
            assert_eq!(blocked.status, "blocked");
            assert_eq!(blocked.status_reason.as_deref(), Some("waiting"));
            assert_eq!(blocked.lock_version, 1);
        }

        let ready_task = ready_task_for_claim(
            &store,
            "t_block_ready_source",
            "block-ready-source",
            "Block ready source",
        )
        .await;
        let blocked_ready = store
            .block_task(
                &ready_task.id,
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_ready_source",
                ),
            )
            .await
            .expect("block ready source");
        assert_eq!(blocked_ready.status, "blocked");
        assert_eq!(blocked_ready.lock_version, 2);

        let review_task =
            ready_task_for_claim(&store, "t_block_review", "block-review", "Block review").await;
        let claimed = store
            .claim_task(
                &review_task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_review",
                    "r_block_review",
                    "e_block_review_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim review source");
        let reviewed = store
            .submit_review_task(
                &review_task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_review"),
                    false,
                    None,
                    400,
                    "e_block_review_submit",
                ),
            )
            .await
            .expect("submit review source");
        let blocked = store
            .block_task(
                &review_task.id,
                block_input(
                    reviewed.lock_version,
                    "reviewer",
                    None,
                    false,
                    "review rejected",
                    500,
                    "e_block_review",
                ),
            )
            .await
            .expect("block review source");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some("review rejected"));
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_review"));
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
    }

    #[tokio::test]
    async fn block_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("block-guards").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_guards", "block-guards", "Block guards").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_guards",
                    "r_block_guards",
                    "e_block_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-block-token"), "e_block_wrong_token"),
            (Some(" claim_block_guards "), "e_block_padded_token"),
            (None, "e_block_missing_token"),
        ] {
            let error = store
                .block_task(
                    &task.id,
                    block_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        "waiting",
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-block-token"));
        }

        let owner_error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        connection
            .execute(
                "UPDATE tasks SET status = 'done' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-blockable");
        let non_source = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_non_source",
                ),
            )
            .await
            .expect_err("done task must fail");
        assert!(matches!(
            non_source,
            StoreError::InvalidTransition(message) if message.contains("cannot block")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running status");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run");
        let missing_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_missing_run",
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
                ("r_block_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("tamper run claim owner");
        let inconsistent_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_inconsistent_run",
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
                ["r_block_guards"],
            )
            .await
            .expect("restore run claim owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("restore active run");

        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for damaged claim");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks after damaged claim");
        let missing_claim = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "admin",
                    None,
                    true,
                    "waiting",
                    500,
                    "e_block_missing_claim",
                ),
            )
            .await
            .expect_err("missing claim expiry must fail");
        assert!(matches!(
            missing_claim,
            StoreError::InvalidTransition(message) if message.contains("active claim")
        ));
        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks to restore claim");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = ?1 WHERE id = ?2",
                (1_300_i64, task.id.as_str()),
            )
            .await
            .expect("restore claim expiry");
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
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_archived",
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

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.claim_owner, claimed.task.claim_owner);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
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
    async fn block_task_force_bypasses_caller_credentials_but_keeps_claim_consistency() {
        let (_directory, store, _path) = store("block-force").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_force", "block-force", "Block force").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_force",
                    "r_block_force",
                    "e_block_force_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "admin",
                    Some("wrong-token"),
                    true,
                    "manual intervention",
                    500,
                    "e_block_force",
                ),
            )
            .await
            .expect("force block task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(
            blocked.status_reason.as_deref(),
            Some("manual intervention")
        );
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_force"));

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, error FROM task_runs WHERE id = ?1",
                    ["r_block_force"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "failed"
        );
        assert_eq!(
            text_value(run.get_value(1).expect("run error"), "run.error").expect("run error text"),
            "manual intervention"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_force"],
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
            text_value(event.get_value(1).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_block_force"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"manual intervention"}"#
        );
    }

    #[tokio::test]
    async fn block_task_event_conflict_rolls_back_task_and_run() {
        let (_directory, store, _path) = store("block-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_block_event_conflict",
            "block-event-conflict",
            "Block event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_event_conflict",
                    "r_block_event_conflict",
                    "e_block_event_claim",
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
                ("e_block_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_event_conflict"),
                    false,
                    "should rollback",
                    500,
                    "e_block_event_conflict",
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
        assert_eq!(unchanged.claim_owner, claimed.task.claim_owner);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_event_conflict"],
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
        assert!(matches!(run.get_value(3).expect("run error"), Value::Null));
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn block_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("block-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_input", "block-input", "Block input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_input_task",
                ),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                block_input(
                    -1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                block_input(1, " ", None, false, "waiting", 500, "e_block_input_actor"),
            ),
            (
                "reason",
                task.id.clone(),
                block_input(1, "worker", None, false, "  ", 500, "e_block_input_reason"),
            ),
            (
                "event_id",
                task.id.clone(),
                block_input(1, "worker", None, false, "waiting", 500, "invalid_event"),
            ),
            (
                "now",
                task.id.clone(),
                block_input(1, "worker", None, false, "waiting", -1, "e_block_input_now"),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .block_task(&task_id, input)
                .await
                .expect_err("invalid block input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        let connection = store.connection().await.expect("connection");
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn block_task_uses_global_task_board_for_event_and_update() {
        let (_directory, store, _path) = store("block-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_block_other', 'block-other', 'Block other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "block-other",
                create_input("t_block_other", Some("block-other"), "Block other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No block plan", "planner", "e_block_other_plan", 100),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_block_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "other board waiting",
                    500,
                    "e_block_other",
                ),
            )
            .await
            .expect("block other-board task");
        assert_eq!(blocked.board_id, "b_block_other");
        assert_eq!(blocked.board_slug, "block-other");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_block_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert!(matches!(
            event.get_value(2).expect("event run"),
            Value::Null
        ));
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.blocked"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"other board waiting"}"#
        );
    }

    #[tokio::test]
    async fn block_task_rejects_non_running_task_with_residual_active_run_without_writes() {
        let (_directory, store, _path) = store("block-residual-run").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_block_residual_run",
                    Some("block-residual-run"),
                    "Residual run",
                ),
            )
            .await
            .expect("create todo task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, 'b_default', ?2, 'running', 'manual', NULL, ?3, ?4, ?5, ?6, ?6, '{}')",
                (
                    "r_block_residual_run",
                    task.id.as_str(),
                    "residual-token",
                    "residual-owner",
                    1_000_i64,
                    300_i64,
                ),
            )
            .await
            .expect("insert residual active run");

        let error = store
            .block_task(
                &task.id,
                block_input(
                    0,
                    "operator",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_residual_run",
                ),
            )
            .await
            .expect_err("residual active run must reject block");
        assert!(matches!(
            error,
            StoreError::InvalidTransition(message) if message.contains("active running run")
        ));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "todo");
        assert_eq!(unchanged.status_reason, None);
        assert_eq!(unchanged.lock_version, task.lock_version);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_residual_run"],
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
        assert!(matches!(run.get_value(3).expect("run error"), Value::Null));
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }
}
