#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn update_step_is_atomic_preserves_null_body_and_emits_strict_payload() {
        let (_directory, store, _path) = store("step-update").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_update_parent", None, "Step update parent"),
            )
            .await
            .expect("create parent");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_update",
                    None,
                    "Original title",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_update_create",
                    "e_step_update_plan",
                    "e_step_update_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        assert_eq!(created.body.as_deref(), Some("body"));
        let updated = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: Some(" Updated title ".into()),
                    body: None,
                    linked_task_id: None,
                    unlink_task: false,
                    position: Some(2048),
                    required: Some(false),
                    updated_by: " reviewer ".into(),
                    event_id: "e_step_update_success".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect("update step");
        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.body.as_deref(), Some("body"));
        assert_eq!(updated.position, 2048);
        assert!(!updated.required);
        assert_eq!(updated.status, "todo");
        assert_eq!(updated.updated_by, "reviewer");

        let parent_after = store
            .get_task_global(&parent.id)
            .await
            .expect("parent after update");
        assert_eq!(parent_after.status, "ready");
        assert_eq!(parent_after.lock_version, 2);
        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT kind, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_step_update_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.step.updated"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "reviewer"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"step_id":"step_update","linked_task_id":null,"position":2048,"required":false,"status":"todo"}"#
        );

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES ('e_step_update_conflict', 'b_default', ?1, NULL, 'test.event', 'tester', '{}', 700)",
                [parent.id.as_str()],
            )
            .await
            .expect("insert conflicting event");
        let conflict = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: Some("Should roll back".into()),
                    body: Some("changed".into()),
                    linked_task_id: None,
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "reviewer".into(),
                    event_id: "e_step_update_conflict".into(),
                    updated_at: 800,
                    expected_lock_version: 2,
                },
            )
            .await
            .expect_err("event conflict must roll back update");
        assert!(matches!(conflict, StoreError::Turso(_)));
        let unchanged = store
            .list_steps(&parent.id)
            .await
            .expect("list after rollback");
        assert_eq!(unchanged.steps[0].title, "Updated title");
        assert_eq!(unchanged.steps[0].body.as_deref(), Some("body"));
        assert_eq!(
            store
                .get_task_global(&parent.id)
                .await
                .expect("parent after rollback")
                .lock_version,
            2
        );
    }

    #[tokio::test]
    async fn update_step_rejects_invalid_links_and_empty_patches() {
        let (_directory, store, _path) = store("step-update-guards").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_update_guard", None, "Step update guard"),
            )
            .await
            .expect("create parent");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_update_guard",
                    None,
                    "Guard",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_update_guard_create",
                    "e_step_update_guard_plan",
                    "e_step_update_guard_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        let self_link = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: None,
                    body: None,
                    linked_task_id: Some(parent.id.clone()),
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "planner".into(),
                    event_id: "e_step_update_guard_self".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect_err("self link must fail");
        assert!(
            matches!(self_link, StoreError::InvalidInput(message) if message.contains("parent"))
        );
        let empty = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: None,
                    body: None,
                    linked_task_id: None,
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "planner".into(),
                    event_id: "e_step_update_guard_empty".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect_err("empty patch must fail");
        assert!(
            matches!(empty, StoreError::InvalidInput(message) if message.contains("at least one"))
        );
    }
}
