#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn list_comments_resolves_task_board_orders_history_and_reads_archived_tasks() {
        let (_directory, store, _path) = store("comment-list-history").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_comment_list_other', 'comment-list-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_list", None, "Comment list"),
            )
            .await
            .expect("create task");
        let other = store
            .create_task(
                "comment-list-other",
                create_input("t_comment_list_other", None, "Other comment list"),
            )
            .await
            .expect("create other task");

        for (id, created_at) in [("c_comment_list_late", 200), ("c_comment_list_b", 100)] {
            store
                .create_comment(
                    &task.id,
                    comment_input(
                        id,
                        None,
                        "operator",
                        "user",
                        None,
                        id,
                        "note",
                        "{}",
                        &format!("e_{id}"),
                        created_at,
                    ),
                )
                .await
                .expect("create comment");
        }
        store
            .create_comment(
                &task.id,
                comment_input(
                    "c_comment_list_a",
                    None,
                    "operator",
                    "user",
                    None,
                    "same timestamp",
                    "note",
                    "{}",
                    "e_c_comment_list_a",
                    100,
                ),
            )
            .await
            .expect("create same-timestamp comment");
        store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_list_other",
                    None,
                    "operator",
                    "user",
                    None,
                    "other board",
                    "note",
                    "{}",
                    "e_c_comment_list_other",
                    50,
                ),
            )
            .await
            .expect("create other-board comment");

        let comments = store.list_comments(&task.id).await.expect("list comments");
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "c_comment_list_a",
                "c_comment_list_b",
                "c_comment_list_late"
            ]
        );
        assert!(
            comments
                .iter()
                .all(|comment| comment.board_id == task.board_id)
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");
        assert_eq!(
            store
                .list_comments(&task.id)
                .await
                .expect("archived history")
                .len(),
            3
        );

        let unknown = store
            .list_comments("t_comment_list_unknown")
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(unknown, StoreError::TaskNotFound(id) if id == "t_comment_list_unknown"));
    }
}
