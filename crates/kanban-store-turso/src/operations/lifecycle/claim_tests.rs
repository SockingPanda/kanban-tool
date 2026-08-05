#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn claim_task_writes_running_task_run_and_event_atomically() {
        let (_directory, store, _path) = store("claim-success").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_claim_success", "claim-success", "Claim success").await;

        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_success",
                    "r_claim_success",
                    "e_claim_success",
                    r#"{"lane":"test"}"#,
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        assert_eq!(claimed.claim_token, "claim_success");
        assert_eq!(claimed.claim_expires_at, 1_300);
        assert_eq!(claimed.task.id, task.id);
        assert_eq!(claimed.task.status, "running");
        assert_eq!(claimed.task.lock_version, 2);
        assert_eq!(claimed.task.claim_token.as_deref(), Some("claim_success"));
        assert_eq!(claimed.task.claim_owner.as_deref(), Some("worker"));
        assert_eq!(claimed.task.claim_expires_at, Some(1_300));
        assert_eq!(claimed.task.last_heartbeat_at, Some(300));
        assert_eq!(claimed.task.started_at, Some(300));
        assert_eq!(
            claimed.task.current_run_id.as_deref(),
            Some("r_claim_success")
        );
        assert_eq!(claimed.run.id, "r_claim_success");
        assert_eq!(claimed.run.board_id, "b_default");
        assert_eq!(claimed.run.task_id, task.id);
        assert_eq!(claimed.run.status, "running");
        assert_eq!(claimed.run.worker_profile.as_deref(), Some("manual"));
        assert_eq!(claimed.run.claim_token, "claim_success");
        assert_eq!(claimed.run.claim_owner, "worker");
        assert_eq!(claimed.run.claim_expires_at, 1_300);
        assert_eq!(claimed.run.started_at, 300);
        assert_eq!(claimed.run.last_heartbeat_at, Some(300));
        assert_eq!(claimed.run.metadata_json, r#"{"lane":"test"}"#);

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 1);
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_claim_success"],
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
            "r_claim_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.claimed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"claim_owner":"worker","metadata":{"lane":"test"}}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            300
        );
    }

    #[tokio::test]
    async fn claim_task_persists_optional_log_path_and_rejects_blank_without_writes() {
        let (_directory, store, _path) = store("claim-log-path").await;
        store.initialize().await.expect("initialize");

        let with_path = ready_task_for_claim(
            &store,
            "t_claim_log_path",
            "claim-log-path",
            "With log path",
        )
        .await;
        let mut with_path_input = claim_input(
            1,
            "worker",
            "claim_log_path",
            "r_claim_log_path",
            "e_claim_log_path",
            "{}",
            300,
            1_000,
        );
        with_path_input.log_path = Some(" /tmp/claim.log ".to_owned());
        let claimed_with_path = store
            .claim_task(&with_path.id, with_path_input)
            .await
            .expect("claim with log path");
        assert_eq!(
            claimed_with_path.run.log_path.as_deref(),
            Some("/tmp/claim.log")
        );

        let none_path =
            ready_task_for_claim(&store, "t_claim_no_log_path", "claim-no-log", "No log path")
                .await;
        let claimed_without_path = store
            .claim_task(
                &none_path.id,
                claim_input(
                    1,
                    "worker",
                    "claim_no_log_path",
                    "r_claim_no_log_path",
                    "e_claim_no_log_path",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim without log path");
        assert_eq!(claimed_without_path.run.log_path, None);

        let invalid_task = ready_task_for_claim(
            &store,
            "t_claim_blank_log_path",
            "claim-blank-log",
            "Blank log path",
        )
        .await;
        let mut invalid_input = claim_input(
            1,
            "worker",
            "claim_blank_log_path",
            "r_claim_blank_log_path",
            "e_claim_blank_log_path",
            "{}",
            300,
            1_000,
        );
        invalid_input.log_path = Some(" \t ".to_owned());
        let error = store
            .claim_task(&invalid_task.id, invalid_input)
            .await
            .expect_err("blank log path must fail");
        assert!(matches!(
            error,
            StoreError::InvalidInput(message) if message.contains("log_path")
        ));
        let unchanged = store
            .get_task_global(&invalid_task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, invalid_task.lock_version);
        let connection = store.connection().await.expect("connection");
        let run_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1",
                    [invalid_task.id.as_str()],
                )
                .await
                .expect("run count query"),
        )
        .await
        .expect("run count row");
        assert_eq!(
            integer_value(run_count.get_value(0).expect("run count"), "run.count")
                .expect("run count integer"),
            0
        );
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [invalid_task.id.as_str()],
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
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
    async fn claim_task_concurrent_callers_have_exactly_one_winner() {
        let (_directory, store, _path) = store("claim-race").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(&store, "t_claim_race", "claim-race", "Claim race").await;
        let first_store = store.clone();
        let second_store = store.clone();
        let (first, second) = tokio::join!(
            first_store.claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker-a",
                    "claim_race_a",
                    "r_claim_race_a",
                    "e_claim_race_a",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            second_store.claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker-b",
                    "claim_race_b",
                    "r_claim_race_b",
                    "e_claim_race_b",
                    "{}",
                    300,
                    1_000,
                ),
            )
        );
        let winners = usize::from(first.is_ok()) + usize::from(second.is_ok());
        assert_eq!(winners, 1);
        for result in [first, second] {
            if let Err(error) = result {
                assert!(matches!(
                    error,
                    StoreError::ClaimConflict(_) | StoreError::InvalidTransition(_)
                ));
            }
        }

        let connection = store.connection().await.expect("connection");
        let active_runs = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1 AND status = 'running'",
                    [task.id.as_str()],
                )
                .await
                .expect("active run count query"),
        )
        .await
        .expect("active run count row");
        assert_eq!(
            integer_value(
                active_runs.get_value(0).expect("active run count"),
                "run.count"
            )
            .expect("active run count integer"),
            1
        );
        let claimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_events.get_value(0).expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            1
        );
        let claimed = store
            .get_task_global(&task.id)
            .await
            .expect("get claimed task");
        assert_eq!(claimed.status, "running");
        assert_eq!(claimed.lock_version, 2);
    }

    #[tokio::test]
    async fn claim_task_validates_token_run_event_metadata_and_ttl_input() {
        let (_directory, store, _path) = store("claim-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_claim_input", "claim-input", "Claim input").await;

        let mut cases = vec![
            (
                "claim_token",
                claim_input(
                    1,
                    "worker",
                    "",
                    "r_claim_input_token",
                    "e_claim_input_token",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "run_id",
                claim_input(
                    1,
                    "worker",
                    "claim_input_run",
                    "bad-run",
                    "e_claim_input_run",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "event_id",
                claim_input(
                    1,
                    "worker",
                    "claim_input_event",
                    "r_claim_input_event",
                    "bad-event",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "owner",
                claim_input(
                    1,
                    " ",
                    "claim_input_owner",
                    "r_claim_input_owner",
                    "e_claim_input_owner",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "claim_expires_at",
                claim_input(
                    1,
                    "worker",
                    "claim_input_ttl",
                    "r_claim_input_ttl",
                    "e_claim_input_ttl",
                    "{}",
                    300,
                    0,
                ),
            ),
        ];
        let mut invalid_profile = claim_input(
            1,
            "worker",
            "claim_input_profile",
            "r_claim_input_profile",
            "e_claim_input_profile",
            "{}",
            300,
            1_000,
        );
        invalid_profile.worker_profile = " ".to_owned();
        cases.push(("worker_profile", invalid_profile));
        let mut invalid_metadata = claim_input(
            1,
            "worker",
            "claim_input_metadata",
            "r_claim_input_metadata",
            "e_claim_input_metadata",
            "{bad",
            300,
            1_000,
        );
        invalid_metadata.metadata_json = "{bad".to_owned();
        cases.push(("metadata_json", invalid_metadata));

        for (field, input) in cases {
            let error = store
                .claim_task(&task.id, input)
                .await
                .expect_err("invalid claim input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
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
    async fn claim_task_rejects_source_plan_spec_schedule_dependency_and_archive_guards() {
        let (_directory, store, _path) = store("claim-guards").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        let unplanned = store
            .create_task(
                "default",
                create_input(
                    "t_claim_unplanned",
                    Some("claim-unplanned"),
                    "Claim unplanned",
                ),
            )
            .await
            .expect("create unplanned task");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [unplanned.id.as_str()],
            )
            .await
            .expect("make unplanned task ready");

        let source = store
            .create_task(
                "default",
                create_input("t_claim_source", Some("claim-source"), "Claim source"),
            )
            .await
            .expect("create source task");
        store
            .mark_execution_plan_not_required(
                &source.id,
                plan_input("No source execution", "planner", "e_claim_source_plan", 100),
            )
            .await
            .expect("mark source plan not required");

        let mut incomplete_input = create_input(
            "t_claim_incomplete",
            Some("claim-incomplete"),
            "Claim incomplete",
        );
        incomplete_input.description = None;
        let incomplete = store
            .create_task("default", incomplete_input)
            .await
            .expect("create incomplete task");
        store
            .mark_execution_plan_not_required(
                &incomplete.id,
                plan_input(
                    "No incomplete execution",
                    "planner",
                    "e_claim_incomplete_plan",
                    100,
                ),
            )
            .await
            .expect("mark incomplete plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [incomplete.id.as_str()],
            )
            .await
            .expect("make incomplete task ready");

        let mut future_input = create_input("t_claim_future", Some("claim-future"), "Claim future");
        future_input.scheduled_at = Some(500);
        let future = store
            .create_task("default", future_input)
            .await
            .expect("create future task");
        store
            .mark_execution_plan_not_required(
                &future.id,
                plan_input("No future execution", "planner", "e_claim_future_plan", 100),
            )
            .await
            .expect("mark future plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [future.id.as_str()],
            )
            .await
            .expect("make future task ready");

        let parent = store
            .create_task(
                "default",
                create_input("t_claim_parent", Some("claim-parent"), "Claim parent"),
            )
            .await
            .expect("create dependency parent");
        let dependency = store
            .create_task(
                "default",
                create_input(
                    "t_claim_dependency",
                    Some("claim-dependency"),
                    "Claim dependency",
                ),
            )
            .await
            .expect("create dependency child");
        store
            .mark_execution_plan_not_required(
                &dependency.id,
                plan_input(
                    "No dependency execution",
                    "planner",
                    "e_claim_dependency_plan",
                    100,
                ),
            )
            .await
            .expect("mark dependency plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [dependency.id.as_str()],
            )
            .await
            .expect("make dependency task ready");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), dependency.id.as_str()),
            )
            .await
            .expect("insert unfinished dependency");

        let archived = store
            .create_task(
                "default",
                create_input("t_claim_archived", Some("claim-archived"), "Claim archived"),
            )
            .await
            .expect("create archived task");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 400 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive claim task");

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_claim_archived', 'claim-archived-board', 'Claim archived board', 1, 1, 450)",
                (),
            )
            .await
            .expect("insert archived claim board");
        let archived_board = store
            .create_task(
                "claim-archived-board",
                create_input(
                    "t_claim_archived_board",
                    Some("claim-archived-board"),
                    "Claim archived board",
                ),
            )
            .await
            .expect("create task on archived claim board");

        let cases = [
            (unplanned.id.as_str(), "execution plan"),
            (source.id.as_str(), "not ready"),
            (incomplete.id.as_str(), "task spec"),
            (future.id.as_str(), "future"),
            (dependency.id.as_str(), "dependency"),
            (archived.id.as_str(), "archived"),
            (archived_board.id.as_str(), "archived"),
        ];
        for (index, (task_id, message)) in cases.into_iter().enumerate() {
            let error = store
                .claim_task(
                    task_id,
                    claim_input(
                        0,
                        "worker",
                        &format!("claim_guard_{index}"),
                        &format!("r_claim_guard_{index}"),
                        &format!("e_claim_guard_{index}"),
                        "{}",
                        100,
                        1_000,
                    ),
                )
                .await
                .expect_err("claim guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        for (task_id, expected_status, expected_plan) in [
            (unplanned.id.as_str(), "ready", "unplanned"),
            (source.id.as_str(), "todo", "not_required"),
            (incomplete.id.as_str(), "ready", "not_required"),
            (future.id.as_str(), "ready", "not_required"),
            (dependency.id.as_str(), "ready", "not_required"),
            (archived.id.as_str(), "archived", "unplanned"),
            (archived_board.id.as_str(), "todo", "unplanned"),
        ] {
            let unchanged = store
                .get_task_global(task_id)
                .await
                .expect("get unchanged guard task");
            assert_eq!(unchanged.status, expected_status, "task {task_id}");
            assert_eq!(unchanged.lock_version, 0, "task {task_id}");
            assert_eq!(
                unchanged.execution_plan_state, expected_plan,
                "task {task_id}"
            );
        }
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.claimed'",
                    (),
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_run_id_conflict_rolls_back_task_update() {
        let (_directory, store, _path) = store("claim-run-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_claim_run_conflict",
            "claim-run-conflict",
            "Claim run conflict",
        )
        .await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, metadata_json) VALUES (?1, 'b_default', ?2, 'succeeded', 'previous', NULL, 'claim_previous', 'previous-worker', 500, 100, '{}')",
                ("r_claim_run_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting run");

        let error = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_run_conflict",
                    "r_claim_run_conflict",
                    "e_claim_run_conflict",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect_err("run id conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back run-conflict task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        assert_eq!(count_rows(&connection, "task_runs").await, 1);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_event_conflict_rolls_back_task_and_run_update() {
        let (_directory, store, _path) = store("claim-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_claim_event_conflict",
            "claim-event-conflict",
            "Claim event conflict",
        )
        .await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_claim_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_event_conflict",
                    "r_claim_event_conflict",
                    "e_claim_event_conflict",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back event-conflict task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("claim-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_claim_other', 'claim-other', 'Claim other', 1, 1)",
                (),
            )
            .await
            .expect("insert other claim board");
        let task = store
            .create_task(
                "claim-other",
                create_input("t_claim_other", Some("claim-other"), "Claim other task"),
            )
            .await
            .expect("create other-board claim task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board claim plan",
                    "planner",
                    "e_claim_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark other-board plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_claim_other_promote", 200),
            )
            .await
            .expect("promote other-board claim task");

        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_other",
                    "r_claim_other",
                    "e_claim_other",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        assert_eq!(claimed.task.board_id, "b_claim_other");
        assert_eq!(claimed.task.board_slug, "claim-other");
        assert_eq!(claimed.run.board_id, "b_claim_other");
        assert_eq!(claimed.run.task_id, task.id);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id FROM task_events WHERE event_id = ?1",
                    ["e_claim_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_claim_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_claim_other"
        );
    }
}
