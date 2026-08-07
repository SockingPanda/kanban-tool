#[cfg(test)]
mod tests {
    use crate::CreateLabelInput;
    use crate::test_support::*;

    #[tokio::test]
    async fn create_task_writes_task_plan_and_event_atomically() {
        let (_directory, store, _path) = store("create").await;
        store.initialize().await.expect("initialize");

        let task = store
            .create_task(
                "default",
                create_input("t_create", Some("create-1"), "Create task"),
            )
            .await
            .expect("create task");
        assert_eq!(task.id, "t_create");
        assert_eq!(task.board_id, "b_default");
        assert_eq!(task.board_slug, "default");
        assert_eq!(task.task_ref, "default#1");
        assert_eq!(task.seq, 1);
        assert_eq!(task.idempotency_key.as_deref(), Some("create-1"));
        assert_eq!(task.title, "Create task");
        assert_eq!(task.status, "todo");
        assert_eq!(task.priority, 1);
        assert_eq!(task.position, 1024);
        assert_eq!(task.lock_version, 0);
        assert_eq!(task.max_retries, Some(2));

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
        let plan = first_row(
            connection
                .query(
                    "SELECT state FROM task_execution_plans WHERE task_id = ?1",
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
        let event = first_row(
            connection
                .query(
                    "SELECT kind, actor, payload_json FROM task_events WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.created"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "tester"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"status":"todo"}"#
        );
    }

    #[tokio::test]
    async fn create_task_attaches_existing_labels_and_dependencies_atomically() {
        let (_directory, store, _path) = store("create-relations").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_create_parent", Some("create-parent"), "Parent"),
            )
            .await
            .expect("parent task");
        store
            .create_board_label(
                "default",
                CreateLabelInput {
                    id: "l_create_bug".to_owned(),
                    name: "bug".to_owned(),
                    color: None,
                    created_at: 100,
                },
            )
            .await
            .expect("label");
        let mut input = create_input("t_create_child", Some("create-child"), "Child");
        input.labels = vec!["bug".to_owned()];
        input.depends_on = vec![parent.id.clone()];
        let child = store
            .create_task("default", input.clone())
            .await
            .expect("child task");
        assert_eq!(child.labels.len(), 1);
        assert_eq!(child.labels[0].name, "bug");
        let dependencies = store
            .list_dependencies(&child.id)
            .await
            .expect("dependencies");
        assert_eq!(dependencies.parents.len(), 1);
        assert_eq!(dependencies.parents[0].id, parent.id);

        let replay = store
            .create_task("b_default", input)
            .await
            .expect("idempotent replay");
        assert_eq!(replay, child);
    }

    #[tokio::test]
    async fn create_task_preserves_allowed_initial_statuses() {
        let (_directory, store, _path) = store("create-statuses").await;
        store.initialize().await.expect("initialize");
        let mut triage = create_input("t_triage", Some("status-triage"), "Triage");
        triage.status = "triage".to_owned();
        let mut scheduled = create_input("t_scheduled", Some("status-scheduled"), "Scheduled");
        scheduled.status = "scheduled".to_owned();

        let triage_task = store
            .create_task("default", triage)
            .await
            .expect("triage create");
        let scheduled_task = store
            .create_task("default", scheduled)
            .await
            .expect("scheduled create");
        assert_eq!(triage_task.status, "triage");
        assert_eq!(scheduled_task.status, "scheduled");

        let mut ready = create_input("t_ready", Some("status-ready"), "Ready");
        ready.status = "ready".to_owned();
        assert!(matches!(
            store.create_task("default", ready).await,
            Err(StoreError::InvalidInput(message)) if message.contains("status")
        ));

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query("SELECT payload_json FROM task_events ORDER BY id ASC", ())
            .await
            .expect("event payload query");
        let mut payloads = Vec::new();
        while let Some(row) = rows.next().await.expect("event payload row") {
            payloads.push(
                text_value(row.get_value(0).expect("event payload"), "event.payload")
                    .expect("event payload text"),
            );
        }
        assert_eq!(
            payloads,
            vec![r#"{"status":"triage"}"#, r#"{"status":"scheduled"}"#]
        );
    }

    #[tokio::test]
    async fn create_task_replays_same_idempotent_payload_without_extra_rows() {
        let (_directory, store, _path) = store("create-replay").await;
        store.initialize().await.expect("initialize");
        let input = create_input("t_replay", Some("replay-1"), "Replay task");
        let first = store
            .create_task("default", input.clone())
            .await
            .expect("first create");
        let mut retry_input = input;
        retry_input.id = "t_replay_retry".to_owned();
        let replay = store
            .create_task("b_default", retry_input)
            .await
            .expect("replay create");
        assert_eq!(first, replay);

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_rejects_same_key_with_different_payload() {
        let (_directory, store, _path) = store("create-conflict").await;
        store.initialize().await.expect("initialize");
        let input = create_input("t_conflict", Some("conflict-1"), "Original");
        store
            .create_task("default", input.clone())
            .await
            .expect("first create");
        let mut changed = input;
        changed.title = "Changed".to_owned();
        let error = store
            .create_task("default", changed)
            .await
            .expect_err("different payload must conflict");
        assert!(matches!(
            error,
            StoreError::IdempotencyConflict {
                board_id,
                key,
                existing_task_id
            } if board_id == "b_default" && key == "conflict-1" && existing_task_id == "t_conflict"
        ));
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_rejects_duplicate_id_without_idempotency_key() {
        let (_directory, store, _path) = store("create-duplicate-id").await;
        store.initialize().await.expect("initialize");
        store
            .create_task("default", create_input("t_duplicate_id", None, "Original"))
            .await
            .expect("first create");

        let error = store
            .create_task("default", create_input("t_duplicate_id", None, "Different"))
            .await
            .expect_err("duplicate task id must conflict");
        assert!(matches!(error, StoreError::TaskConflict(task_id) if task_id == "t_duplicate_id"));

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_rejects_duplicate_id_with_different_idempotency_key() {
        let (_directory, store, _path) = store("create-duplicate-id-key").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_duplicate_id_key", Some("key-1"), "Original"),
            )
            .await
            .expect("first create");

        let error = store
            .create_task(
                "default",
                create_input("t_duplicate_id_key", Some("key-2"), "Different"),
            )
            .await
            .expect_err("duplicate task id with a different key must conflict");
        assert!(matches!(
            error,
            StoreError::TaskConflict(task_id) if task_id == "t_duplicate_id_key"
        ));

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_does_not_classify_event_constraint_as_task_conflict() {
        let (_directory, store, _path) = store("create-event-conflict").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', NULL, NULL, 'test.event', 'tester', '{}', 1)",
                ["e_event_conflict_created"],
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .create_task(
                "default",
                create_input("t_event_conflict", None, "Event conflict"),
            )
            .await
            .expect_err("event constraint must remain a storage error");
        assert!(matches!(
            error,
            StoreError::Turso(turso::Error::Constraint(message))
                if message.starts_with("UNIQUE constraint failed: task_events.event_id")
        ));
        assert_eq!(count_rows(&connection, "tasks").await, 0);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 0);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_assigns_monotonic_board_local_sequences() {
        let (_directory, store, _path) = store("create-seq").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task("default", create_input("t_seq_1", Some("seq-1"), "First"))
            .await
            .expect("first create");
        let second = store
            .create_task(
                "b_default",
                create_input("t_seq_2", Some("seq-2"), "Second"),
            )
            .await
            .expect("second create");
        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert_ne!(first.id, second.id);
    }

    #[tokio::test]
    async fn create_task_reports_missing_board() {
        let (_directory, store, _path) = store("create-missing-board").await;
        store.initialize().await.expect("initialize");
        let error = store
            .create_task(
                "missing",
                create_input("t_missing_board", Some("missing-board"), "Missing"),
            )
            .await
            .expect_err("missing board must fail");
        assert!(matches!(error, StoreError::BoardNotFound(selector) if selector == "missing"));
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 0);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 0);
        assert_eq!(count_rows(&connection, "task_events").await, 0);
    }

    #[tokio::test]
    async fn create_task_failure_does_not_leave_partial_rows() {
        let (_directory, store, _path) = store("create-rollback").await;
        store.initialize().await.expect("initialize");
        let mut invalid = create_input("t_invalid_json", Some("invalid-json"), "Invalid");
        invalid.metadata_json = "{not-json".to_owned();
        assert!(store.create_task("default", invalid).await.is_err());

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 0);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 0);
        assert_eq!(count_rows(&connection, "task_events").await, 0);
    }
}
