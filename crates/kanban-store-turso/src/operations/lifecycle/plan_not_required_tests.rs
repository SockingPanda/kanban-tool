#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn mark_execution_plan_not_required_writes_plan_and_event() {
        let (_directory, store, _path) = store("plan-not-required-success").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_success",
                    Some("plan-success"),
                    "Plan success",
                ),
            )
            .await
            .expect("create task");

        let plan = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No execution needed", "planner", "e_plan_success", 100),
            )
            .await
            .expect("mark plan not required");
        assert_eq!(
            plan,
            TaskExecutionPlanRecord {
                board_id: "b_default".to_owned(),
                task_id: task.id.clone(),
                state: "not_required".to_owned(),
                reason: Some("No execution needed".to_owned()),
                updated_by: "planner".to_owned(),
                updated_at: 100,
            }
        );

        let connection = store.connection().await.expect("connection");
        let persisted = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = ?1 AND task_id = ?2",
                    ("b_default", task.id.as_str()),
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(persisted.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "not_required"
        );
        assert_eq!(
            text_value(persisted.get_value(1).expect("plan reason"), "plan.reason")
                .expect("plan reason text"),
            "No execution needed"
        );
        assert_eq!(
            text_value(
                persisted.get_value(2).expect("plan actor"),
                "plan.updated_by"
            )
            .expect("plan actor text"),
            "planner"
        );
        assert_eq!(
            integer_value(
                persisted.get_value(3).expect("plan updated_at"),
                "plan.updated_at"
            )
            .expect("plan updated_at integer"),
            100
        );
        let event = first_row(
            connection
                .query(
                    "SELECT event_id, board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE kind = 'task.execution_plan.not_required'",
                    (),
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event id"), "event.event_id")
                .expect("event id text"),
            "e_plan_success"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.execution_plan.not_required"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "planner"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"state":"not_required"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            100
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_retries_without_extra_event_and_updates_reason() {
        let (_directory, store, _path) = store("plan-not-required-retry").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_retry",
                    Some("plan-retry"),
                    "Plan retry",
                ),
            )
            .await
            .expect("create task");

        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("First reason", "planner", "e_plan_retry_first", 100),
            )
            .await
            .expect("first mark");
        let retry = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Updated reason", "reviewer", "e_plan_retry_second", 200),
            )
            .await
            .expect("retry mark");
        assert_eq!(retry.state, "not_required");
        assert_eq!(retry.reason.as_deref(), Some("Updated reason"));
        assert_eq!(retry.updated_by, "reviewer");
        assert_eq!(retry.updated_at, 200);

        let connection = store.connection().await.expect("connection");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
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
            1
        );
        let event = first_row(
            connection
                .query(
                    "SELECT event_id, actor, created_at FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event id"), "event.event_id")
                .expect("event id text"),
            "e_plan_retry_first"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "planner"
        );
        assert_eq!(
            integer_value(
                event.get_value(2).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            100
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_rejects_steps_archived_and_unknown_without_partial_write()
     {
        let (_directory, store, _path) = store("plan-not-required-reject").await;
        store.initialize().await.expect("initialize");

        let unknown = store
            .mark_execution_plan_not_required(
                "t_plan_not_required_unknown",
                plan_input("Unknown", "planner", "e_plan_unknown", 100),
            )
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(
            unknown,
            StoreError::TaskNotFound(task_id) if task_id == "t_plan_not_required_unknown"
        ));

        let with_step = store
            .create_task(
                "default",
                create_input("t_plan_not_required_step", Some("plan-step"), "Plan step"),
            )
            .await
            .expect("create task with step");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'Existing step', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_plan_not_required", with_step.id.as_str()),
            )
            .await
            .expect("insert existing step");
        let step_error = store
            .mark_execution_plan_not_required(
                &with_step.id,
                plan_input("Has steps", "planner", "e_plan_step", 200),
            )
            .await
            .expect_err("task with steps must fail");
        assert!(matches!(
            step_error,
            StoreError::InvalidInput(message) if message.contains("steps")
        ));

        let archived = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_archived",
                    Some("plan-archived"),
                    "Plan archived",
                ),
            )
            .await
            .expect("create archived task");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived_error = store
            .mark_execution_plan_not_required(
                &archived.id,
                plan_input("Archived", "planner", "e_plan_archived", 300),
            )
            .await
            .expect_err("archived task must fail");
        assert!(matches!(
            archived_error,
            StoreError::InvalidInput(message) if message.contains("archived")
        ));

        let mut rows = connection
            .query(
                "SELECT task_id, state FROM task_execution_plans WHERE task_id IN (?1, ?2) ORDER BY task_id",
                (with_step.id.as_str(), archived.id.as_str()),
            )
            .await
            .expect("plan query");
        let mut states = Vec::new();
        while let Some(row) = rows.next().await.expect("plan row") {
            states.push((
                text_value(row.get_value(0).expect("plan task"), "plan.task_id")
                    .expect("plan task text"),
                text_value(row.get_value(1).expect("plan state"), "plan.state")
                    .expect("plan state text"),
            ));
        }
        assert_eq!(
            states,
            vec![
                (archived.id.clone(), "unplanned".to_owned()),
                (with_step.id.clone(), "unplanned".to_owned()),
            ]
        );
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.execution_plan.not_required'",
                    (),
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
    async fn mark_execution_plan_not_required_rejects_archived_board_without_partial_write() {
        let (_directory, store, _path) = store("plan-not-required-archived-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_archived', 'archived-board', 'Archived board', 1, 1, 350)",
                (),
            )
            .await
            .expect("insert archived board");
        let task = store
            .create_task(
                "archived-board",
                create_input(
                    "t_plan_not_required_archived_board",
                    Some("plan-archived-board"),
                    "Archived board task",
                ),
            )
            .await
            .expect("create task on archived board");

        let error = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Archived board", "planner", "e_plan_archived_board", 400),
            )
            .await
            .expect_err("archived board must fail");
        assert!(matches!(
            error,
            StoreError::InvalidInput(message) if message.contains("archived")
        ));

        let plan = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = ?1 AND task_id = ?2",
                    ("b_archived", task.id.as_str()),
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(plan.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "unplanned"
        );
        assert!(matches!(
            plan.get_value(1).expect("plan reason"),
            Value::Null
        ));
        assert_eq!(
            text_value(plan.get_value(2).expect("plan actor"), "plan.updated_by")
                .expect("plan actor text"),
            "tester"
        );
        let generated_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE board_id = ?1 AND task_id = ?2 AND kind = 'task.execution_plan.not_required'",
                    ("b_archived", task.id.as_str()),
                )
                .await
                .expect("generated event count query"),
        )
        .await
        .expect("generated event count row");
        assert_eq!(
            integer_value(
                generated_event_count
                    .get_value(0)
                    .expect("generated event count"),
                "event.count",
            )
            .expect("generated event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_event_conflict_rolls_back_plan_update() {
        let (_directory, store, _path) = store("plan-not-required-conflict").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_conflict",
                    Some("plan-conflict"),
                    "Plan conflict",
                ),
            )
            .await
            .expect("create task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_plan_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let baseline_plan = first_row(
            connection
                .query(
                    "SELECT updated_at FROM task_execution_plans WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("baseline plan query"),
        )
        .await
        .expect("baseline plan row");
        let baseline_updated_at = integer_value(
            baseline_plan
                .get_value(0)
                .expect("baseline plan updated_at"),
            "plan.updated_at",
        )
        .expect("baseline plan updated_at integer");

        let error = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Should roll back", "planner", "e_plan_conflict", 400),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let plan = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(plan.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "unplanned"
        );
        assert!(matches!(
            plan.get_value(1).expect("plan reason"),
            Value::Null
        ));
        assert_eq!(
            text_value(plan.get_value(2).expect("plan actor"), "plan.updated_by")
                .expect("plan actor text"),
            "tester"
        );
        assert_eq!(
            integer_value(
                plan.get_value(3).expect("plan updated_at"),
                "plan.updated_at"
            )
            .expect("plan updated_at integer"),
            baseline_updated_at
        );
        let generated_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
                )
                .await
                .expect("generated event count query"),
        )
        .await
        .expect("generated event count row");
        assert_eq!(
            integer_value(
                generated_event_count
                    .get_value(0)
                    .expect("generated event count"),
                "event.count",
            )
            .expect("generated event count integer"),
            0
        );
        assert_eq!(count_rows(&connection, "task_events").await, 2);
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_uses_task_board() {
        let (_directory, store, _path) = store("plan-not-required-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        let task = store
            .create_task(
                "other",
                create_input(
                    "t_plan_not_required_other",
                    Some("plan-other"),
                    "Other plan",
                ),
            )
            .await
            .expect("create other-board task");

        let plan = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "Other board does not need execution",
                    "planner",
                    "e_plan_other",
                    500,
                ),
            )
            .await
            .expect("mark other-board plan");
        assert_eq!(plan.board_id, "b_other");
        assert_eq!(plan.task_id, task.id);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id FROM task_events WHERE event_id = ?1",
                    ["e_plan_other"],
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
    }
}
