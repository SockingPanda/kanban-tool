#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn dependency_remove_is_atomic_idempotent_and_preserves_task_state() {
        let (_directory, store, _path) = store("dependency-remove").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_remove_parent", None, "Remove parent"),
            )
            .await
            .expect("create parent");
        let child = store
            .create_task(
                "default",
                create_input("t_dependency_remove_child", None, "Remove child"),
            )
            .await
            .expect("create child");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'done', completed_at = 400 WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("finish parent");
        store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_add".to_owned(),
                    recompute_event_id: "e_dependency_remove_recompute".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("add dependency");
        let before = store
            .get_task_global(&child.id)
            .await
            .expect("read child before remove");
        let removed = store
            .remove_dependency(
                &child.id,
                &parent.id,
                RemoveDependencyInput {
                    actor: " remover ".to_owned(),
                    event_id: "e_dependency_removed".to_owned(),
                    now: 600,
                },
            )
            .await
            .expect("remove dependency");
        assert!(removed.removed);
        assert!(removed.dependencies.parents.is_empty());
        assert!(removed.dependencies.edges.is_empty());
        let after = store
            .get_task_global(&child.id)
            .await
            .expect("read child after remove");
        assert_eq!(after.status, before.status);
        assert_eq!(after.lock_version, before.lock_version);
        let removed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.removed'",
                    [child.id.as_str()],
                )
                .await
                .expect("removed event count"),
        )
        .await
        .expect("removed event count row");
        assert_eq!(
            integer_value(
                removed_events.get_value(0).expect("removed count"),
                "event.count"
            )
            .expect("removed count integer"),
            1
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_dependency_removed"],
                )
                .await
                .expect("removed event query"),
        )
        .await
        .expect("removed event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "remover"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            format!(r#"{{"parent_task_id":"{}"}}"#, parent.id)
        );

        let replay = store
            .remove_dependency(
                &child.id,
                &parent.id,
                RemoveDependencyInput {
                    actor: "replay".to_owned(),
                    event_id: "e_dependency_removed_replay".to_owned(),
                    now: 700,
                },
            )
            .await
            .expect("missing edge replay");
        assert!(!replay.removed);
        assert_eq!(replay.dependencies, removed.dependencies);
        let replay_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.removed'",
                    [child.id.as_str()],
                )
                .await
                .expect("replay event count"),
        )
        .await
        .expect("replay event count row");
        assert_eq!(
            integer_value(
                replay_events.get_value(0).expect("replay count"),
                "event.count"
            )
            .expect("replay count integer"),
            1
        );

        let second_parent = store
            .create_task(
                "default",
                create_input("t_dependency_remove_parent_two", None, "Remove parent two"),
            )
            .await
            .expect("create second parent");
        store
            .add_dependency(
                &child.id,
                &second_parent.id,
                AddDependencyInput {
                    expected_child_lock_version: after.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_add_two".to_owned(),
                    recompute_event_id: "e_dependency_remove_recompute_two".to_owned(),
                    now: 800,
                },
            )
            .await
            .expect("add second dependency");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'test.event', 'tester', '{}', 900)",
                ("e_dependency_remove_conflict", child.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let conflict = store
            .remove_dependency(
                &child.id,
                &second_parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_conflict".to_owned(),
                    now: 1_000,
                },
            )
            .await
            .expect_err("event conflict must roll back edge deletion");
        assert!(matches!(conflict, StoreError::Turso(_)));
        let remaining = store
            .list_dependencies(&child.id)
            .await
            .expect("list after rollback");
        assert_eq!(remaining.parents.len(), 1);
        assert_eq!(remaining.parents[0].id, second_parent.id);
        assert_eq!(
            store
                .get_task_global(&child.id)
                .await
                .expect("read child after rollback")
                .lock_version,
            after.lock_version
        );

        let unknown = store
            .remove_dependency(
                "t_dependency_remove_unknown",
                &parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_unknown".to_owned(),
                    now: 1_100,
                },
            )
            .await
            .expect_err("unknown child must fail");
        assert!(
            matches!(unknown, StoreError::TaskNotFound(id) if id == "t_dependency_remove_unknown")
        );

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_dependency_remove_other', 'dependency-remove-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other_parent = store
            .create_task(
                "dependency-remove-other",
                create_input("t_dependency_remove_other_parent", None, "Other parent"),
            )
            .await
            .expect("create other-board parent");
        let cross_board = store
            .remove_dependency(
                &child.id,
                &other_parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_cross_board".to_owned(),
                    now: 1_200,
                },
            )
            .await
            .expect_err("cross-board removal must fail");
        assert!(matches!(
            cross_board,
            StoreError::InvalidInput(message) if message.contains("cross-board")
        ));
    }
}
