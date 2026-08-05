#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn dependency_create_list_recomputes_atomically_and_rejects_cycles() {
        let (_directory, store, _path) = store("dependency-create-list").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_parent", None, "Dependency parent"),
            )
            .await
            .expect("create parent");
        let child = store
            .create_task(
                "default",
                create_input("t_dependency_child", None, "Dependency child"),
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

        let first = store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: " tester ".to_owned(),
                    event_id: "e_dependency_added".to_owned(),
                    recompute_event_id: "e_dependency_recomputed".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("add dependency");
        assert!(first.added);
        assert_eq!(first.dependencies.task.id, child.id);
        assert_eq!(
            first
                .dependencies
                .parents
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec![parent.id.as_str()]
        );
        assert!(first.dependencies.children.is_empty());
        assert_eq!(first.dependencies.edges.len(), 1);
        assert_eq!(first.dependencies.edges[0].parent.id, parent.id);
        assert_eq!(first.dependencies.edges[0].child.id, child.id);
        assert_eq!(first.dependencies.task.status, "todo");

        let replay = store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "different actor".to_owned(),
                    event_id: "e_dependency_added_retry".to_owned(),
                    recompute_event_id: "e_dependency_recomputed_retry".to_owned(),
                    now: 900,
                },
            )
            .await
            .expect("dependency replay");
        assert!(!replay.added);
        assert_eq!(replay.dependencies, first.dependencies);
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind IN ('dependency.added', 'task.recomputed')",
                    [child.id.as_str()],
                )
                .await
                .expect("dependency event count"),
        )
        .await
        .expect("dependency event count row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "event.count")
                .expect("event count integer"),
            1
        );

        let listed = store
            .list_dependencies(&child.id)
            .await
            .expect("list dependencies");
        assert_eq!(listed, first.dependencies);

        let cycle = store
            .add_dependency(
                &parent.id,
                &child.id,
                AddDependencyInput {
                    expected_child_lock_version: parent.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_cycle".to_owned(),
                    recompute_event_id: "e_dependency_cycle_recompute".to_owned(),
                    now: 1_000,
                },
            )
            .await
            .expect_err("cycle must be rejected");
        assert!(matches!(cycle, StoreError::DependencyCycle(message) if message.contains("cycle")));
        let edge_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_dependencies WHERE board_id = 'b_default'",
                    (),
                )
                .await
                .expect("edge count"),
        )
        .await
        .expect("edge count row");
        assert_eq!(
            integer_value(edge_count.get_value(0).expect("edge count"), "edge.count")
                .expect("edge count integer"),
            1
        );

        let unknown = store
            .list_dependencies("t_dependency_unknown")
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(unknown, StoreError::TaskNotFound(id) if id == "t_dependency_unknown"));
    }

    #[tokio::test]
    async fn dependency_create_enforces_board_and_running_guards_and_demotes_ready_children() {
        let (_directory, store, _path) = store("dependency-create-guards").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_dependency_other', 'dependency-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_guard_parent", None, "Guard parent"),
            )
            .await
            .expect("create parent");
        let ready_child = store
            .create_task(
                "default",
                create_input("t_dependency_guard_ready", None, "Ready child"),
            )
            .await
            .expect("create ready child");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [ready_child.id.as_str()],
            )
            .await
            .expect("make child ready");
        let ready_child = store
            .get_task_global(&ready_child.id)
            .await
            .expect("read ready child");
        let demoted = store
            .add_dependency(
                &ready_child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: ready_child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_demoted".to_owned(),
                    recompute_event_id: "e_dependency_demoted_recompute".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("ready child should be demoted");
        assert!(demoted.added);
        assert_eq!(demoted.dependencies.task.status, "todo");
        assert_eq!(
            demoted.dependencies.task.lock_version,
            ready_child.lock_version + 1
        );
        let recompute_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.recomputed'",
                    [ready_child.id.as_str()],
                )
                .await
                .expect("demotion recompute events"),
        )
        .await
        .expect("demotion recompute events row");
        assert_eq!(
            integer_value(
                recompute_events.get_value(0).expect("recompute count"),
                "event.count",
            )
            .expect("recompute count integer"),
            1
        );
        let dependency_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.added'",
                    [ready_child.id.as_str()],
                )
                .await
                .expect("demotion dependency events"),
        )
        .await
        .expect("demotion dependency events row");
        assert_eq!(
            integer_value(
                dependency_events.get_value(0).expect("dependency count"),
                "event.count",
            )
            .expect("dependency count integer"),
            1
        );

        let running_child = store
            .create_task(
                "default",
                create_input("t_dependency_guard_running", None, "Running child"),
            )
            .await
            .expect("create running child");
        connection
            .execute(
                "UPDATE tasks SET status = 'running', claim_token = 'token-running', claim_owner = 'tester', claim_expires_at = 999999 WHERE id = ?1",
                [running_child.id.as_str()],
            )
            .await
            .expect("make child running");
        let running_error = store
            .add_dependency(
                &running_child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: running_child.lock_version,
                    target_child_status: "running".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_running".to_owned(),
                    recompute_event_id: "e_dependency_running_recompute".to_owned(),
                    now: 600,
                },
            )
            .await
            .expect_err("running child cannot receive unfinished parent");
        assert!(matches!(
            running_error,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));

        let other_parent = store
            .create_task(
                "dependency-other",
                create_input("t_dependency_other_parent", None, "Other parent"),
            )
            .await
            .expect("create other parent");
        let cross_board = store
            .add_dependency(
                &ready_child.id,
                &other_parent.id,
                AddDependencyInput {
                    expected_child_lock_version: ready_child.lock_version + 1,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_cross_board".to_owned(),
                    recompute_event_id: "e_dependency_cross_board_recompute".to_owned(),
                    now: 700,
                },
            )
            .await
            .expect_err("cross-board dependency must be rejected");
        assert!(matches!(
            cross_board,
            StoreError::InvalidInput(message) if message.contains("cross-board")
        ));
    }
}
