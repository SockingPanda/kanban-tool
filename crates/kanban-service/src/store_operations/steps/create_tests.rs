#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn create_step_plans_parent_recomputes_status_and_lists_in_canonical_order() {
        let (_directory, store, _path) = store("step-create-list").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_parent", None, "Step parent"),
            )
            .await
            .expect("create parent");
        let first = store
            .create_step(
                &parent.id,
                step_input(
                    "step_first",
                    Some("step-replay"),
                    "  First step  ",
                    None,
                    "operator",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_first",
                    "e_step_plan",
                    "e_step_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        assert_eq!(first.id, "step_first");
        assert_eq!(first.title, "First step");
        assert_eq!(first.position, 1024);
        assert_eq!(first.status, "todo");
        let listed = store.list_steps(&parent.id).await.expect("list steps");
        assert_eq!(listed.steps.len(), 1);
        assert_eq!(listed.steps[0], first);
        assert_eq!(listed.execution_plan.state, "planned");
        let updated_parent = store
            .get_task_global(&parent.id)
            .await
            .expect("read recomputed parent");
        assert_eq!(updated_parent.status, "ready");
        assert_eq!(updated_parent.lock_version, 1);

        let connection = store.connection().await.expect("connection");
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind IN ('task.step.created', 'task.execution_plan.planned', 'task.recomputed')",
                    [parent.id.as_str()],
                )
                .await
                .expect("step events"),
        )
        .await
        .expect("step event row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "events")
                .expect("event integer"),
            3
        );
    }

    #[tokio::test]
    async fn create_step_replays_same_payload_without_events_and_rejects_conflicts_or_archived_parents()
     {
        let (_directory, store, _path) = store("step-idempotency").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_idempotent", None, "Step idempotent"),
            )
            .await
            .expect("create parent");
        let first_input = step_input(
            "step_idempotent",
            Some("step-replay"),
            "step",
            None,
            "operator",
            parent.lock_version,
            "unplanned",
            "ready",
            "e_step_idempotent",
            "e_step_idempotent_plan",
            "e_step_idempotent_recompute",
            500,
        );
        let first = store
            .create_step(&parent.id, first_input.clone())
            .await
            .expect("first step");
        let replay = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_retry_id".into(),
                    event_id: "e_step_retry".into(),
                    plan_event_id: "e_step_retry_plan".into(),
                    recompute_event_id: "e_step_retry_recompute".into(),
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    created_at: 900,
                    ..first_input.clone()
                },
            )
            .await
            .expect("idempotent replay");
        assert_eq!(replay, first);
        let changed = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    title: "different".into(),
                    id: "step_changed".into(),
                    event_id: "e_step_changed".into(),
                    plan_event_id: "e_step_changed_plan".into(),
                    recompute_event_id: "e_step_changed_recompute".into(),
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    created_at: 1_000,
                    ..first_input.clone()
                },
            )
            .await
            .expect_err("changed payload must conflict");
        assert!(
            matches!(changed, StoreError::IdempotencyConflict { key, .. } if key == "step-replay")
        );
        let events = first_row(
            store
                .connection()
                .await
                .expect("connection")
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.step.created'",
                    [parent.id.as_str()],
                )
                .await
                .expect("step event count"),
        )
        .await
        .expect("step event row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "events")
                .expect("event integer"),
            1
        );

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 1000 WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("archive parent");
        assert_eq!(
            store
                .list_steps(&parent.id)
                .await
                .expect("archived list")
                .steps
                .len(),
            1
        );
        let archived_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_archived".into(),
                    idempotency_key: None,
                    title: "archived".into(),
                    body: None,
                    linked_task_id: None,
                    position: Some(2048),
                    required: true,
                    created_by: "operator".into(),
                    event_id: "e_step_archived".into(),
                    plan_event_id: "e_step_archived_plan".into(),
                    recompute_event_id: "e_step_archived_recompute".into(),
                    created_at: 1_100,
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    target_status: "ready".into(),
                },
            )
            .await
            .expect_err("archived create must fail");
        assert!(
            matches!(archived_error, StoreError::InvalidTransition(message) if message.contains("archived"))
        );
    }

    #[tokio::test]
    async fn create_step_rejects_cross_board_and_self_links_and_rolls_back_event_conflicts() {
        let (_directory, store, _path) = store("step-guards").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_guard_parent", None, "Step guard parent"),
            )
            .await
            .expect("create parent");

        let self_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_self_link".into(),
                    idempotency_key: None,
                    linked_task_id: Some(parent.id.clone()),
                    event_id: "e_step_self_link".into(),
                    plan_event_id: "e_step_self_link_plan".into(),
                    recompute_event_id: "e_step_self_link_recompute".into(),
                    expected_lock_version: parent.lock_version,
                    expected_plan_state: "unplanned".into(),
                    target_status: "ready".into(),
                    created_at: 600,
                    ..step_input(
                        "step_self_link",
                        None,
                        "self link",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_self_link",
                        "e_step_self_link_plan",
                        "e_step_self_link_recompute",
                        600,
                    )
                },
            )
            .await
            .expect_err("self link must fail");
        assert!(matches!(
            self_error,
            StoreError::InvalidInput(message) if message.contains("parent")
        ));

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_step_other', 'step-other', 'Other steps', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let linked = store
            .create_task(
                "step-other",
                create_input("t_step_other", None, "Other linked task"),
            )
            .await
            .expect("create linked task");
        let cross_board_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_cross_board".into(),
                    idempotency_key: None,
                    linked_task_id: Some(linked.id),
                    event_id: "e_step_cross_board".into(),
                    plan_event_id: "e_step_cross_board_plan".into(),
                    recompute_event_id: "e_step_cross_board_recompute".into(),
                    created_at: 700,
                    ..step_input(
                        "step_cross_board",
                        None,
                        "cross board",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_cross_board",
                        "e_step_cross_board_plan",
                        "e_step_cross_board_recompute",
                        700,
                    )
                },
            )
            .await
            .expect_err("cross-board link must fail");
        assert!(matches!(
            cross_board_error,
            StoreError::InvalidInput(message) if message.contains("parent board")
        ));

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES ('e_step_rollback', 'b_default', ?1, NULL, 'test.event', 'tester', '{}', 800)",
                [parent.id.as_str()],
            )
            .await
            .expect("insert conflicting event");
        let rollback_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_rollback".into(),
                    idempotency_key: None,
                    event_id: "e_step_rollback".into(),
                    plan_event_id: "e_step_rollback_plan".into(),
                    recompute_event_id: "e_step_rollback_recompute".into(),
                    created_at: 900,
                    ..step_input(
                        "step_rollback",
                        None,
                        "rollback",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_rollback",
                        "e_step_rollback_plan",
                        "e_step_rollback_recompute",
                        900,
                    )
                },
            )
            .await
            .expect_err("event conflict must abort the transaction");
        assert!(matches!(rollback_error, StoreError::Turso(_)));
        assert!(
            store
                .list_steps(&parent.id)
                .await
                .expect("list after rollback")
                .steps
                .is_empty()
        );
        let parent_after = store
            .get_task_global(&parent.id)
            .await
            .expect("parent after rollback");
        assert_eq!(parent_after.status, "todo");
        assert_eq!(parent_after.lock_version, parent.lock_version);
        assert_eq!(parent_after.execution_plan_state, "unplanned");
        let leftovers = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND event_id IN ('e_step_rollback_plan', 'e_step_rollback_recompute')",
                    [parent.id.as_str()],
                )
                .await
                .expect("rollback events query"),
        )
        .await
        .expect("rollback event row");
        assert_eq!(
            integer_value(leftovers.get_value(0).expect("leftover count"), "events")
                .expect("leftover event integer"),
            0
        );
    }
}
