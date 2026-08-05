#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn get_task_global_returns_complete_task_record() {
        let (_directory, store, _path) = store("show-global").await;
        store.initialize().await.expect("initialize");
        let created = store
            .create_task(
                "default",
                create_input("t_show", Some("show-1"), "Show task"),
            )
            .await
            .expect("create task");

        let shown = store
            .get_task_global("t_show")
            .await
            .expect("get global task");
        assert_eq!(shown, created);
        assert_eq!(shown.board_id, "b_default");
        assert_eq!(shown.board_slug, "default");
        assert_eq!(shown.task_ref, "default#1");
        assert_eq!(shown.execution_plan_state, "unplanned");
        assert_eq!(shown.unfinished_parent_count, 0);
        assert!(shown.labels.is_empty());
    }

    #[tokio::test]
    async fn get_task_global_rejects_invalid_and_unknown_ids() {
        let (_directory, store, _path) = store("show-errors").await;
        store.initialize().await.expect("initialize");

        let invalid = store
            .get_task_global("default#1")
            .await
            .expect_err("board-local ref must be rejected");
        assert!(
            matches!(invalid, StoreError::InvalidInput(message) if message.contains("task id"))
        );

        let unknown = store
            .get_task_global("t_unknown")
            .await
            .expect_err("unknown global id must be not found");
        assert!(matches!(unknown, StoreError::TaskNotFound(task_id) if task_id == "t_unknown"));
    }

    #[tokio::test]
    async fn get_task_global_resolves_the_correct_board_without_board_local_lookup() {
        let (_directory, store, _path) = store("show-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        let default_task = store
            .create_task(
                "default",
                create_input("t_default_show", Some("show-default"), "Default task"),
            )
            .await
            .expect("create default task");
        let other_task = store
            .create_task(
                "b_other",
                create_input("t_other_show", Some("show-other"), "Other task"),
            )
            .await
            .expect("create other task");

        let shown_default = store
            .get_task_global(&default_task.id)
            .await
            .expect("get default global task");
        let shown_other = store
            .get_task_global(&other_task.id)
            .await
            .expect("get other global task");
        assert_eq!(shown_default.board_slug, "default");
        assert_eq!(shown_default.seq, 1);
        assert_eq!(shown_other.board_id, "b_other");
        assert_eq!(shown_other.board_slug, "other");
        assert_eq!(shown_other.seq, 1);
    }
}
