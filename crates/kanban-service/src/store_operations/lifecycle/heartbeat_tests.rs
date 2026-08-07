#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn heartbeat_task_extends_task_and_run_and_writes_note_event_atomically() {
        let (_directory, store, _path) = store("heartbeat-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_success",
            "heartbeat-success",
            "Heartbeat success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_success",
                    "r_heartbeat_success",
                    "e_heartbeat_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");

        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_success",
                    "e_heartbeat_success",
                    Some("still alive"),
                    500,
                    1_500,
                ),
            )
            .await
            .expect("heartbeat task");
        assert_eq!(heartbeated.status, "running");
        assert_eq!(heartbeated.claim_expires_at, Some(1_500));
        assert_eq!(heartbeated.last_heartbeat_at, Some(500));
        assert_eq!(heartbeated.updated_at, 500);
        assert_eq!(heartbeated.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id = ?1",
                    ["r_heartbeat_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            integer_value(run.get_value(0).expect("run expiry"), "run.expiry")
                .expect("run expiry integer"),
            1_500
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run heartbeat"), "run.heartbeat")
                .expect("run heartbeat integer"),
            500
        );

        let event = first_row(
            connection
                .query(
                    "SELECT kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_heartbeat_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.heartbeat"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"note":"still alive"}"#
        );
    }

    #[tokio::test]
    async fn heartbeat_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("heartbeat-guards").await;
        store.initialize().await.expect("initialize");

        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_running",
            "heartbeat-guards-running",
            "Heartbeat guards running",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_guards",
                    "r_heartbeat_guards",
                    "e_heartbeat_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim running task");

        let wrong_token = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "secret-token-that-must-not-leak",
                    "e_heartbeat_wrong_token",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("wrong token must fail");
        assert!(matches!(wrong_token, StoreError::ClaimTokenMismatch));
        assert!(
            !wrong_token
                .to_string()
                .contains("secret-token-that-must-not-leak")
        );

        let padded_token = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    " claim_heartbeat_guards ",
                    "e_heartbeat_padded_token",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("padded token must not be normalized");
        assert!(matches!(padded_token, StoreError::ClaimTokenMismatch));

        let wrong_owner = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_heartbeat_guards",
                    "e_heartbeat_wrong_owner",
                    None,
                    500,
                    1_500,
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
        assert_eq!(after_credentials.lock_version, claimed.task.lock_version);
        assert_eq!(
            after_credentials.claim_expires_at,
            claimed.task.claim_expires_at
        );
        assert_eq!(
            after_credentials.last_heartbeat_at,
            claimed.task.last_heartbeat_at
        );

        let ready = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_ready",
            "heartbeat-guards-ready",
            "Heartbeat guards ready",
        )
        .await;
        let non_running = store
            .heartbeat_task(
                &ready.id,
                heartbeat_input(
                    ready.lock_version,
                    "worker",
                    "claim_never_created",
                    "e_heartbeat_non_running",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));

        let missing_run = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_missing_run",
            "heartbeat-guards-missing-run",
            "Heartbeat guards missing run",
        )
        .await;
        let missing_claim = store
            .claim_task(
                &missing_run.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_missing_run",
                    "r_heartbeat_missing_run",
                    "e_heartbeat_missing_run_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim missing-run task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [missing_run.id.as_str()],
            )
            .await
            .expect("remove current run id");
        let missing_run_error = store
            .heartbeat_task(
                &missing_run.id,
                heartbeat_input(
                    missing_claim.task.lock_version,
                    "worker",
                    "claim_heartbeat_missing_run",
                    "e_heartbeat_missing_run",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run_error,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        let unchanged_missing_run = store
            .get_task_global(&missing_run.id)
            .await
            .expect("get missing-run task");
        assert_eq!(
            unchanged_missing_run.lock_version,
            missing_claim.task.lock_version
        );
        assert_eq!(
            unchanged_missing_run.claim_expires_at,
            missing_claim.task.claim_expires_at
        );
        assert_eq!(
            unchanged_missing_run.last_heartbeat_at,
            missing_claim.task.last_heartbeat_at
        );

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_heartbeat_guards"],
            )
            .await
            .expect("damage active run status");
        let damaged_run_error = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_guards",
                    "e_heartbeat_damaged_run",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("damaged run must fail");
        assert!(matches!(
            damaged_run_error,
            StoreError::InvalidTransition(_)
        ));

        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.heartbeat'",
                    (),
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_validates_input_without_opening_a_mutation_path() {
        let (_directory, store, _path) = store("heartbeat-input").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_input",
            "heartbeat-input",
            "Heartbeat input",
        )
        .await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                heartbeat_input(1, "worker", "claim_input", "e_input", None, 300, 1_500),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                heartbeat_input(
                    -1,
                    "worker",
                    "claim_input",
                    "e_input_version",
                    None,
                    300,
                    1_500,
                ),
            ),
            (
                "actor",
                task.id.clone(),
                heartbeat_input(1, " ", "claim_input", "e_input_actor", None, 300, 1_500),
            ),
            (
                "claim_token",
                task.id.clone(),
                heartbeat_input(1, "worker", " ", "e_input_token", None, 300, 1_500),
            ),
            (
                "event_id",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "input_event", None, 300, 1_500),
            ),
            (
                "now",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "e_input_now", None, -1, 1_500),
            ),
            (
                "claim_expires_at",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "e_input_expiry", None, 300, 300),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .heartbeat_task(&task_id, input)
                .await
                .expect_err("invalid heartbeat input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let heartbeat_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.heartbeat'",
                    (),
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                heartbeat_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("heartbeat-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_event_conflict",
            "heartbeat-event-conflict",
            "Heartbeat event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_event_conflict",
                    "r_heartbeat_event_conflict",
                    "e_heartbeat_event_claim",
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
                ("e_heartbeat_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_event_conflict",
                    "e_heartbeat_event_conflict",
                    Some("must roll back"),
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_expires_at, claimed.task.claim_expires_at);
        assert_eq!(unchanged.last_heartbeat_at, claimed.task.last_heartbeat_at);
        let run = first_row(
            connection
                .query(
                    "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id = ?1",
                    ["r_heartbeat_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            integer_value(run.get_value(0).expect("run expiry"), "run.expiry")
                .expect("run expiry integer"),
            claimed.run.claim_expires_at
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run heartbeat"), "run.heartbeat")
                .expect("run heartbeat integer"),
            claimed.run.last_heartbeat_at.expect("claimed heartbeat")
        );
        let heartbeat_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.heartbeat'",
                    [task.id.as_str()],
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                heartbeat_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("heartbeat-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_heartbeat_other', 'heartbeat-other', 'Heartbeat other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "heartbeat-other",
                create_input(
                    "t_heartbeat_other",
                    Some("heartbeat-other"),
                    "Heartbeat other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board heartbeat plan",
                    "planner",
                    "e_heartbeat_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_heartbeat_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_heartbeat_other",
                    "r_heartbeat_other",
                    "e_heartbeat_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_heartbeat_other",
                    "e_heartbeat_other",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect("heartbeat other-board task");
        assert_eq!(heartbeated.board_id, "b_heartbeat_other");
        assert_eq!(heartbeated.board_slug, "heartbeat-other");

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_heartbeat_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_heartbeat_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_heartbeat_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"note":null}"#
        );
    }
}
