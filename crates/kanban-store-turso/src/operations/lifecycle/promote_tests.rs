#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn promote_task_todo_writes_ready_and_event() {
        let (_directory, store, _path) = store("promote-todo").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_promote_todo", Some("promote-todo"), "Promote todo"),
            )
            .await
            .expect("create task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No execution plan", "planner", "e_promote_plan", 100),
            )
            .await
            .expect("mark plan not required");

        let promoted = store
            .promote_task(&task.id, promote_input(0, "promoter", "e_promoted", 200))
            .await
            .expect("promote task");
        assert_eq!(promoted.id, task.id);
        assert_eq!(promoted.board_id, "b_default");
        assert_eq!(promoted.board_slug, "default");
        assert_eq!(promoted.task_ref, "default#1");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.status_reason, None);
        assert_eq!(promoted.lock_version, 1);
        assert_eq!(promoted.updated_at, 200);
        assert_eq!(promoted.execution_plan_state, "not_required");
        assert!(!promoted.dependency_blocked);
        assert_eq!(promoted.unfinished_parent_count, 0);
        assert!(promoted.labels.is_empty());

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_promoted"],
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
            text_value(event.get_value(2).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.promoted"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "promoter"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(5).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            200
        );
    }

    #[tokio::test]
    async fn promote_task_scheduled_when_due_writes_ready() {
        let (_directory, store, _path) = store("promote-scheduled-due").await;
        store.initialize().await.expect("initialize");
        let mut input = create_input(
            "t_promote_scheduled",
            Some("promote-scheduled"),
            "Promote scheduled",
        );
        input.status = "scheduled".to_owned();
        input.scheduled_at = Some(100);
        let task = store
            .create_task("default", input)
            .await
            .expect("create scheduled task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No scheduled execution",
                    "planner",
                    "e_promote_scheduled_plan",
                    100,
                ),
            )
            .await
            .expect("mark scheduled plan not required");

        let promoted = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promoted_scheduled", 100),
            )
            .await
            .expect("promote due scheduled task");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.scheduled_at, Some(100));
        assert_eq!(promoted.lock_version, 1);
        assert_eq!(promoted.updated_at, 100);

        let connection = store.connection().await.expect("connection");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.promoted' AND payload_json = '{\"to_status\":\"ready\"}'",
                    [task.id.as_str()],
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            1
        );
    }

    #[tokio::test]
    async fn promote_task_rejects_source_and_readiness_guards_without_partial_write() {
        let (_directory, store, _path) = store("promote-guards").await;
        store.initialize().await.expect("initialize");
        let unplanned = store
            .create_task(
                "default",
                create_input(
                    "t_promote_unplanned",
                    Some("promote-unplanned"),
                    "Unplanned",
                ),
            )
            .await
            .expect("create unplanned task");

        let mut triage_input =
            create_input("t_promote_source", Some("promote-source"), "Invalid source");
        triage_input.status = "triage".to_owned();
        let source = store
            .create_task("default", triage_input)
            .await
            .expect("create source task");

        let mut incomplete_input = create_input(
            "t_promote_incomplete",
            Some("promote-incomplete"),
            "Incomplete",
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
                    "No execution plan",
                    "planner",
                    "e_promote_incomplete_plan",
                    100,
                ),
            )
            .await
            .expect("mark incomplete plan not required");

        let mut future_input = create_input(
            "t_promote_future",
            Some("promote-future"),
            "Future scheduled",
        );
        future_input.status = "scheduled".to_owned();
        future_input.scheduled_at = Some(500);
        let future = store
            .create_task("default", future_input)
            .await
            .expect("create future task");
        store
            .mark_execution_plan_not_required(
                &future.id,
                plan_input(
                    "No future execution",
                    "planner",
                    "e_promote_future_plan",
                    100,
                ),
            )
            .await
            .expect("mark future plan not required");

        let parent = store
            .create_task(
                "default",
                create_input("t_promote_parent", Some("promote-parent"), "Parent"),
            )
            .await
            .expect("create dependency parent");
        let child = store
            .create_task(
                "default",
                create_input("t_promote_child", Some("promote-child"), "Child"),
            )
            .await
            .expect("create dependency child");
        store
            .mark_execution_plan_not_required(
                &child.id,
                plan_input("No child execution", "planner", "e_promote_child_plan", 100),
            )
            .await
            .expect("mark child plan not required");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), child.id.as_str()),
            )
            .await
            .expect("insert unfinished dependency");

        let cases = [
            (unplanned.id.as_str(), "execution plan"),
            (source.id.as_str(), "cannot promote from triage"),
            (incomplete.id.as_str(), "task spec"),
            (future.id.as_str(), "future"),
            (child.id.as_str(), "dependency"),
        ];
        for (index, (task_id, message)) in cases.into_iter().enumerate() {
            let error = store
                .promote_task(
                    task_id,
                    promote_input(0, "promoter", &format!("e_promote_guard_{index}"), 100),
                )
                .await
                .expect_err("readiness guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        for (task_id, expected_status, expected_plan) in [
            (unplanned.id.as_str(), "todo", "unplanned"),
            (source.id.as_str(), "triage", "unplanned"),
            (incomplete.id.as_str(), "todo", "not_required"),
            (future.id.as_str(), "scheduled", "not_required"),
            (child.id.as_str(), "todo", "not_required"),
        ] {
            let unchanged = store
                .get_task_global(task_id)
                .await
                .expect("get unchanged task");
            assert_eq!(unchanged.status, expected_status, "task {task_id}");
            assert_eq!(unchanged.lock_version, 0, "task {task_id}");
            assert_eq!(
                unchanged.execution_plan_state, expected_plan,
                "task {task_id}"
            );
        }
        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.promoted'",
                    (),
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_rejects_archived_task_board_and_stale_version_without_partial_write() {
        let (_directory, store, _path) = store("promote-archive-stale").await;
        store.initialize().await.expect("initialize");
        let archived_task = store
            .create_task(
                "default",
                create_input(
                    "t_promote_archived_task",
                    Some("promote-archived-task"),
                    "Archived task",
                ),
            )
            .await
            .expect("create archived task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [archived_task.id.as_str()],
            )
            .await
            .expect("archive task");

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_promote_archived', 'promote-archived', 'Archived promote board', 1, 1, 350)",
                (),
            )
            .await
            .expect("insert archived board");
        let archived_board_task = store
            .create_task(
                "promote-archived",
                create_input(
                    "t_promote_archived_board",
                    Some("promote-archived-board"),
                    "Archived board task",
                ),
            )
            .await
            .expect("create task on archived board");

        let stale = store
            .create_task(
                "default",
                create_input("t_promote_stale", Some("promote-stale"), "Stale task"),
            )
            .await
            .expect("create stale task");
        store
            .mark_execution_plan_not_required(
                &stale.id,
                plan_input("No stale execution", "planner", "e_promote_stale_plan", 100),
            )
            .await
            .expect("mark stale plan not required");

        for (task_id, expected_lock_version, message) in [
            (archived_task.id.as_str(), 0_i64, "archived task or board"),
            (
                archived_board_task.id.as_str(),
                0_i64,
                "archived task or board",
            ),
            (stale.id.as_str(), 1_i64, "lock_version mismatch"),
        ] {
            let error = store
                .promote_task(
                    task_id,
                    promote_input(
                        expected_lock_version,
                        "promoter",
                        &format!("e_promote_archive_stale_{}", task_id),
                        100,
                    ),
                )
                .await
                .expect_err("archive/stale guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        let archived_task_after = store
            .get_task_global(&archived_task.id)
            .await
            .expect("get archived task");
        assert_eq!(archived_task_after.status, "archived");
        assert_eq!(archived_task_after.lock_version, 0);
        let archived_board_task_after = store
            .get_task_global(&archived_board_task.id)
            .await
            .expect("get archived board task");
        assert_eq!(archived_board_task_after.status, "todo");
        assert_eq!(archived_board_task_after.lock_version, 0);
        let stale_after = store
            .get_task_global(&stale.id)
            .await
            .expect("get stale task");
        assert_eq!(stale_after.status, "todo");
        assert_eq!(stale_after.lock_version, 0);
        assert_eq!(stale_after.execution_plan_state, "not_required");

        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.promoted'",
                    (),
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_event_conflict_rolls_back_status_update() {
        let (_directory, store, _path) = store("promote-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_promote_event_conflict",
                    Some("promote-event-conflict"),
                    "Promote event conflict",
                ),
            )
            .await
            .expect("create task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No conflicting execution",
                    "planner",
                    "e_promote_conflict_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_promote_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promote_conflict", 200),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "todo");
        assert_eq!(unchanged.lock_version, 0);
        assert_eq!(unchanged.updated_at, task.updated_at);
        assert_eq!(unchanged.execution_plan_state, "not_required");

        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.promoted'",
                    [task.id.as_str()],
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_uses_global_task_board() {
        let (_directory, store, _path) = store("promote-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "other",
                create_input("t_promote_other", Some("promote-other"), "Other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board execution",
                    "planner",
                    "e_promote_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark other-board plan not required");

        let promoted = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promote_other", 200),
            )
            .await
            .expect("promote other-board task");
        assert_eq!(promoted.board_id, "b_other");
        assert_eq!(promoted.board_slug, "other");
        assert_eq!(promoted.task_ref, "other#1");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.lock_version, 1);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_promote_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
    }

    #[tokio::test]
    async fn promote_task_validates_global_input() {
        let (_directory, store, _path) = store("promote-input").await;
        store.initialize().await.expect("initialize");

        let invalid_id = store
            .promote_task(
                "default#1",
                promote_input(0, "promoter", "e_promote_input", 100),
            )
            .await
            .expect_err("board-local id must fail");
        assert!(matches!(
            invalid_id,
            StoreError::InvalidInput(message) if message.contains("task id")
        ));

        let invalid_version = store
            .promote_task(
                "t_promote_input",
                promote_input(-1, "promoter", "e_promote_input_version", 100),
            )
            .await
            .expect_err("negative version must fail");
        assert!(matches!(
            invalid_version,
            StoreError::InvalidInput(message) if message.contains("expected_lock_version")
        ));

        let invalid_actor = store
            .promote_task(
                "t_promote_input",
                promote_input(0, " ", "e_promote_input_actor", 100),
            )
            .await
            .expect_err("empty actor must fail");
        assert!(matches!(
            invalid_actor,
            StoreError::InvalidInput(message) if message.contains("actor")
        ));

        let invalid_event = store
            .promote_task(
                "t_promote_input",
                promote_input(0, "promoter", "promote_input_event", 100),
            )
            .await
            .expect_err("invalid event id must fail");
        assert!(matches!(
            invalid_event,
            StoreError::InvalidInput(message) if message.contains("event_id")
        ));

        let invalid_time = store
            .promote_task(
                "t_promote_input",
                promote_input(0, "promoter", "e_promote_input_time", -1),
            )
            .await
            .expect_err("negative time must fail");
        assert!(matches!(
            invalid_time,
            StoreError::InvalidInput(message) if message.contains("updated_at")
        ));
    }
}
